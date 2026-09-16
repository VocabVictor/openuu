//! Start-up and command-line entry points of configuration provisioning
//! (docs/server-config-provisioning.md §3). The parsing and the tables live
//! in `base::config::provision`; this file only decides when to call them.

use base::config::{builtin, provision};
use hbb_common::log;
use std::path::Path;

/// Built-in defaults, then the persisted copy of the last import, then a
/// changed `openuu-config.json` next to the executable. Runs before the custom
/// client file so `custom.txt` keeps the last word.
pub fn at_startup() {
    builtin::apply();
    provision::reload_persisted();
    match provision::import_exe_dir_file() {
        Some(Ok(report)) => log::info!(
            "event=config_import_file applied={} locked={} ignored={}",
            report.applied.len(),
            report.locked.len(),
            report.ignored.len()
        ),
        Some(Err(err)) => log::error!("event=config_import_file_error err={err}"),
        None => {}
    }
}

/// `--import-config <path>`: a `.json` file takes the provisioning parser, any
/// other path the legacy `OpenUU.toml` / `OpenUU2.toml` copy. When a legacy
/// path is given but a provisioning file sits next to the executable, the
/// provisioning file wins and the conflict is logged.
pub fn import_config_path(path: &str, import_legacy: impl FnOnce(&str)) {
    if is_provisioning_file(path) {
        import_json(path);
        return;
    }
    if let Some(json) = exe_dir_file() {
        log::warn!(
            "event=config_import_conflict legacy={path} json={} decision=json",
            json.display()
        );
        import_json(&json.to_string_lossy());
        return;
    }
    import_legacy(path);
}

fn is_provisioning_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

fn exe_dir_file() -> Option<std::path::PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join(provision::EXE_DIR_FILE);
    path.is_file().then_some(path)
}

fn import_json(path: &str) {
    match std::fs::read_to_string(path)
        .map_err(|e| e.into())
        .and_then(|text| provision::apply(&text, provision::Source::Cli))
    {
        Ok(report) => log::info!(
            "event=config_import path={path} applied={} locked={} secrets={} ignored={}",
            report.applied.len(),
            report.locked.len(),
            report.secrets.len(),
            report.ignored.len()
        ),
        Err(err) => log::error!("event=config_import_error path={path} err={err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_paths_are_provisioning_files() {
        assert!(is_provisioning_file(r"C:\x\openuu-config.json"));
        assert!(is_provisioning_file("/tmp/a.JSON"));
        assert!(!is_provisioning_file("/tmp/OpenUU.toml"));
        assert!(!is_provisioning_file("noext"));
    }

    #[test]
    fn a_legacy_path_falls_through_when_no_json_is_next_to_the_exe() {
        let mut called = None;
        import_config_path("/tmp/OpenUU.toml", |p| called = Some(p.to_owned()));
        // The test binary has no openuu-config.json beside it.
        assert_eq!(called.as_deref(), Some("/tmp/OpenUU.toml"));
    }
}
