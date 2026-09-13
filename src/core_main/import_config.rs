use hbb_common::{
    config::{load_path, store_path, Config, Config2},
    get_exe_time, get_modified_time, log,
};
use std::{path::Path, time::SystemTime};

pub(super) fn import_config(path: &str) {
    let path2 = path.replace(".toml", "2.toml");
    let path2 = Path::new(&path2);
    let path = Path::new(path);
    log::info!("import config from {:?} and {:?}", path, path2);
    import_config_files(path, path2, &Config::file(), &Config2::file(), get_exe_time());
}

/// Copies the user's `<app>.toml` / `<app>2.toml` into the service's config files.
///
/// The two files are decided independently: `<app>.toml` (id, key pair, password) is
/// taken only when it is non-empty, newer than the target and older than the
/// executable, as before. `<app>2.toml` (options,
/// including the custom server) is taken when the target does not exist, when the
/// target has never been configured with a server, or when the source is newer; an
/// existing target keeps its own keys and only receives the source's options on top.
fn import_config_files(src: &Path, src2: &Path, dst: &Path, dst2: &Path, exe_time: SystemTime) {
    let config: Config = load_path(src.into());
    if config.is_empty() {
        log::info!("Empty source config, skipped");
    } else if get_modified_time(src) > get_modified_time(dst)
        && get_modified_time(src) < exe_time
    {
        if let Err(err) = store_path(dst.into(), config) {
            log::error!("Failed to write {:?}: {err}", dst);
        }
    }
    let config2: Config2 = load_path(src2.into());
    if !dst2.exists() {
        if let Err(err) = store_path(dst2.into(), config2) {
            log::error!("Failed to write {:?}: {err}", dst2);
        }
        return;
    }
    let mut target: Config2 = load_path(dst2.into());
    let unconfigured = !target.options.contains_key("custom-rendezvous-server");
    if unconfigured || get_modified_time(src2) > get_modified_time(dst2) {
        target.options.extend(config2.options);
        if let Err(err) = store_path(dst2.into(), target) {
            log::error!("Failed to write {:?}: {err}", dst2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, fs, time::Duration};

    const SRC_CONFIG: &str = "id = '123456789'\nkey_pair = [[1, 2, 3], [4, 5, 6]]\n";

    fn options(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn config2_with(pairs: &[(&str, &str)]) -> Config2 {
        let mut c = Config2::default();
        c.options = options(pairs);
        c
    }

    fn server_options() -> Vec<(&'static str, &'static str)> {
        vec![
            ("custom-rendezvous-server", "1.2.3.4:21116"),
            ("relay-server", "1.2.3.4:21117"),
            ("api-server", "http://1.2.3.4:21114"),
            ("key", "abc="),
        ]
    }

    struct Case {
        dir: std::path::PathBuf,
    }

    impl Drop for Case {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    impl Case {
        fn new() -> Self {
            static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("openuu-import-{}-{seq}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("src.toml"), SRC_CONFIG).unwrap();
            store_path(dir.join("src2.toml"), config2_with(&server_options())).unwrap();
            Case { dir }
        }
        fn p(&self, name: &str) -> std::path::PathBuf {
            self.dir.join(name)
        }
        fn run(&self) {
            let exe_time = SystemTime::now() + Duration::from_secs(3600);
            import_config_files(&self.p("src.toml"), &self.p("src2.toml"), &self.p("dst.toml"), &self.p("dst2.toml"), exe_time);
        }
        fn dst2(&self) -> Config2 {
            load_path(self.p("dst2.toml"))
        }
    }

    #[test]
    fn copies_both_files_when_the_target_does_not_exist() {
        let case = Case::new();
        case.run();
        let dst: Config = load_path(case.p("dst.toml"));
        assert!(!dst.is_empty());
        assert_eq!(case.dst2().options, options(&server_options()));
    }

    #[test]
    fn fills_an_unconfigured_target_and_keeps_its_other_keys() {
        let case = Case::new();
        store_path(case.p("dst2.toml"), config2_with(&[("pinned-windows-session", "bob")])).unwrap();
        case.run();
        let mut expected = options(&server_options());
        expected.insert("pinned-windows-session".into(), "bob".into());
        assert_eq!(case.dst2().options, expected);
    }

    #[test]
    fn leaves_a_newer_configured_target_alone() {
        let case = Case::new();
        std::thread::sleep(Duration::from_millis(20));
        let existing = [("custom-rendezvous-server", "9.9.9.9:21116"), ("pinned-windows-session", "bob")];
        store_path(case.p("dst2.toml"), config2_with(&existing)).unwrap();
        case.run();
        assert_eq!(case.dst2().options, options(&existing));
    }

    #[test]
    fn merges_into_an_older_configured_target() {
        let case = Case::new();
        let existing = [("custom-rendezvous-server", "9.9.9.9:21116"), ("pinned-windows-session", "bob")];
        store_path(case.p("dst2.toml"), config2_with(&existing)).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        store_path(case.p("src2.toml"), config2_with(&server_options())).unwrap();
        case.run();
        let mut expected = options(&server_options());
        expected.insert("pinned-windows-session".into(), "bob".into());
        assert_eq!(case.dst2().options, expected);
    }

    #[test]
    fn an_empty_source_config_skips_only_itself() {
        let case = Case::new();
        fs::write(case.p("src.toml"), "").unwrap();
        case.run();
        assert!(!case.p("dst.toml").exists());
        assert_eq!(case.dst2().options, options(&server_options()));
    }
}
