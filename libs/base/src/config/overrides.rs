//! What this run says, over what the machine has stored.
//!
//! ```text
//! OPENUU_OPT_<KEY>   environment   highest -- this run, this shell
//! --option <k>=<v>   command line
//! <user's config>    stored files
//! openuu-config.json provisioning
//! built-in           builtin.rs    lowest
//! ```
//!
//! The top two answer "what does this build do if I change this one value"
//! without editing a file the app also writes. Nothing they set is stored, so
//! the next start without them is the old behaviour. They also beat a
//! provisioned `locked` key; `docs/config-precedence.md` argues that one.
//!
//! Applying and reporting are separate because these have to be applied before
//! anything reads a setting, and `init_log` runs much later in `core_main`.

use super::{keys, provision};
use hbb_common::{config, log};

/// `OPENUU_OPT_ALLOW_D3D_RENDER=N` sets `allow-d3d-render`.
pub const ENV_PREFIX: &str = "OPENUU_OPT_";
/// `--option allow-d3d-render=N`, repeatable.
pub const FLAG: &str = "--option";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum From {
    Cli,
    Env,
}

impl From {
    fn name(&self) -> &'static str {
        match self {
            From::Cli => "cli",
            From::Env => "env",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    pub key: String,
    pub value: String,
    pub from: From,
}

/// The variable that sets `key`: upper case, `-` becomes `_`.
///
/// Only used in this direction. Back the other way is ambiguous --
/// `allow-d3d-render` and `allow_d3d_render` give the same variable -- so the
/// environment is read by asking each known key whether its variable is set.
pub fn var(key: &str) -> String {
    format!("{ENV_PREFIX}{}", key.to_uppercase().replace('-', "_"))
}

/// Everything a provisioning file can carry; a key outside these has no table.
fn keys() -> impl Iterator<Item = &'static str> {
    keys::KEYS_SETTINGS
        .iter()
        .chain(keys::KEYS_DISPLAY_SETTINGS.iter())
        .chain(keys::KEYS_LOCAL_SETTINGS.iter())
        .copied()
}

/// Splits `--option k=v` out, returning the rest for the caller to go on with.
pub fn from_args(args: Vec<String>) -> (Vec<Set>, Vec<String>) {
    let (mut found, mut rest) = (Vec::new(), Vec::new());
    let mut want_value = false;
    for arg in args {
        if want_value {
            want_value = false;
            push(&mut found, &arg);
        } else if arg == FLAG {
            want_value = true;
        } else if let Some(pair) = arg.strip_prefix(&format!("{FLAG}=")) {
            push(&mut found, pair);
        } else {
            rest.push(arg);
        }
    }
    (found, rest)
}

fn push(out: &mut Vec<Set>, pair: &str) {
    match pair.split_once('=') {
        Some((k, v)) if !k.is_empty() => out.push(Set {
            key: k.to_owned(),
            value: v.to_owned(),
            from: From::Cli,
        }),
        _ => log::warn!("event=config_override_bad value={pair}"),
    }
}

/// One variable per known key. A variable that looks like ours but matches no
/// key is ignored: far more likely to be someone else's than a typo of ours.
fn from_env() -> Vec<Set> {
    keys()
        .filter_map(|key| {
            std::env::var(var(key)).ok().map(|value| Set {
                key: key.to_owned(),
                value,
                from: From::Env,
            })
        })
        .collect()
}

/// What `apply` did, logged once the logger exists.
#[derive(Debug, Default)]
pub struct Report {
    ok: Vec<Set>,
    no: Vec<(Set, &'static str)>,
}

impl Report {
    /// Values are logged: these are keys a deployment file may carry, and
    /// knowing what a run was told is the point of having said it.
    pub fn log(&self) {
        for s in &self.ok {
            log::info!(
                "event=config_override key={} value={} source={}",
                s.key,
                s.value,
                s.from.name()
            );
        }
        for (s, why) in &self.no {
            log::warn!(
                "event=config_override_refused key={} source={} reason={why}",
                s.key,
                s.from.name()
            );
        }
    }
}

/// Command line first, environment second: one table, last write wins.
pub fn apply(cli: Vec<Set>) -> Report {
    let mut report = Report::default();
    for s in cli.into_iter().chain(from_env()) {
        match set(&s) {
            None => report.ok.push(s),
            Some(why) => report.no.push((s, why)),
        }
    }
    report
}

/// `None` when applied, `Some(reason)` when not.
fn set(s: &Set) -> Option<&'static str> {
    // Secrets have their own stores, and an override is the easiest way to
    // leave a password in a shell history or a process listing.
    if provision::is_secret_key(&s.key) {
        return Some("secret");
    }
    let table = if provision::is_setting(&s.key) {
        &*config::OVERWRITE_SETTINGS
    } else if provision::is_display_setting(&s.key) {
        &*config::OVERWRITE_DISPLAY_SETTINGS
    } else if provision::is_local_setting(&s.key) {
        &*config::OVERWRITE_LOCAL_SETTINGS
    } else {
        return Some("no such key");
    };
    table.write().unwrap().insert(s.key.clone(), s.value.clone());
    None
}

#[cfg(test)]
mod tests;
