//! Installing a [`Provision`] into the configuration tables and keeping a copy
//! so locked keys survive a restart (docs/server-config-provisioning.md §3).

use super::*;
use hbb_common::{
    config::{self, Config, LocalConfig},
    log,
    sha2::{Digest, Sha256},
};
use std::collections::HashSet;

/// Sanitized copy of the last imported file, reloaded at every start.
pub const PERSISTED_FILE: &str = "imported-config.json";
/// LocalConfig option holding the SHA-256 of the last imported source text.
pub const HASH_KEY: &str = "imported-config-hash";
/// File name looked for next to the executable.
pub const EXE_DIR_FILE: &str = "openuu-config.json";
/// The one secret with its own store (hashed by Config::set_permanent_password).
pub const KEY_PERMANENT_PASSWORD: &str = "permanent-password";

/// Parses, validates and installs `text`, then persists it (trusted sources only).
pub fn apply(text: &str, source: Source) -> ResultType<Report> {
    let p = parse(text)?;
    validate(&p)?;
    let report = install(&p, source);
    if source.trusted() {
        persist(&p, text)?;
    }
    log::info!(
        "event=config_import source={source:?} applied={} locked={} secrets={} ignored={}",
        report.applied.len(),
        report.locked.len(),
        report.secrets.len(),
        report.ignored.len()
    );
    Ok(report)
}

/// Reloads the persisted copy into the tables. Called at start-up after the
/// built-in defaults and before the custom client file, so a `custom.txt`
/// keeps the last word as today.
pub fn reload_persisted() {
    let path = Config::path(PERSISTED_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    match parse(&text).and_then(|p| {
        validate(&p)?;
        Ok(install(&p, Source::File))
    }) {
        Ok(r) => log::info!(
            "event=config_reload file={} applied={} locked={}",
            path.display(),
            r.applied.len(),
            r.locked.len()
        ),
        Err(err) => log::warn!("event=config_reload_error file={} err={err}", path.display()),
    }
}

/// Imports `<exe dir>/openuu-config.json` unless its content was imported
/// already. `None` when there is no file or nothing changed.
pub fn import_exe_dir_file() -> Option<ResultType<Report>> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let path = dir.join(EXE_DIR_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    if sha256_hex(&text) == LocalConfig::get_option(HASH_KEY) {
        return None;
    }
    log::info!("event=config_import_file path={}", path.display());
    Some(apply(&text, Source::File))
}

fn persist(p: &Provision, source_text: &str) -> ResultType<()> {
    let copy = serde_json::to_string_pretty(&sanitized(p))?;
    std::fs::write(Config::path(PERSISTED_FILE), copy)?;
    LocalConfig::set_option(HASH_KEY.to_owned(), sha256_hex(source_text));
    Ok(())
}

pub(crate) fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Installs the provision into the default / overwrite tables. Locked keys and
/// secrets are honoured only for trusted sources; everything else is reported.
pub(crate) fn install(p: &Provision, source: Source) -> Report {
    let mut report = Report::default();
    let locked: HashSet<&str> = if source.trusted() {
        p.locked.iter().map(String::as_str).collect()
    } else {
        if !p.locked.is_empty() {
            report.warnings.push("locked keys ignored: not a trusted source".into());
        }
        HashSet::new()
    };
    let entries = server_options(p)
        .into_iter()
        .chain(p.options.iter().map(|(k, v)| (k.clone(), v.clone())));
    for (key, value) in entries {
        if is_secret_key(&key) {
            if source.trusted() && apply_secret(&key, &value) {
                report.secrets.push(key);
            } else {
                report.warnings.push(format!("{key}: secret ignored from this source"));
                report.ignored.push(key);
            }
            continue;
        }
        let is_locked = locked.contains(key.as_str());
        let table = if is_setting(&key) {
            Some(if is_locked { &*config::OVERWRITE_SETTINGS } else { &*config::DEFAULT_SETTINGS })
        } else if is_display_setting(&key) {
            Some(if is_locked {
                &*config::OVERWRITE_DISPLAY_SETTINGS
            } else {
                &*config::DEFAULT_DISPLAY_SETTINGS
            })
        } else if is_buildin_setting(&key) {
            Some(&*config::BUILTIN_SETTINGS)
        } else {
            None
        };
        match table {
            Some(table) => {
                table.write().unwrap().insert(key.clone(), value);
                if is_locked {
                    report.locked.push(key);
                } else {
                    report.applied.push(key);
                }
            }
            None => {
                log::warn!("event=config_import_unknown_key key={key}");
                report.ignored.push(key);
            }
        }
    }
    for (key, value) in &p.local {
        if is_secret_key(key) {
            report.warnings.push(format!("{key}: secrets are not accepted in local"));
            report.ignored.push(key.clone());
        } else if is_local_setting(key) {
            let table = if locked.contains(key.as_str()) {
                &*config::OVERWRITE_LOCAL_SETTINGS
            } else {
                &*config::DEFAULT_LOCAL_SETTINGS
            };
            table.write().unwrap().insert(key.clone(), value.clone());
            if locked.contains(key.as_str()) {
                report.locked.push(key.clone());
            } else {
                report.applied.push(key.clone());
            }
        } else {
            log::warn!("event=config_import_unknown_key section=local key={key}");
            report.ignored.push(key.clone());
        }
    }
    report
}

/// Secrets go to their own stores, never to the default tables. The value is
/// not logged.
fn apply_secret(key: &str, value: &str) -> bool {
    if key == KEY_PERMANENT_PASSWORD {
        return Config::set_permanent_password(value);
    }
    Config::set_option(key.to_owned(), value.to_owned());
    true
}
