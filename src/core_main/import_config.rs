use hbb_common::log;

pub(super) fn import_config(path: &str) {
    use hbb_common::{config::*, get_exe_time, get_modified_time};
    let path2 = path.replace(".toml", "2.toml");
    let path2 = std::path::Path::new(&path2);
    let path = std::path::Path::new(path);
    log::info!("import config from {:?} and {:?}", path, path2);
    let config: Config = load_path(path.into());
    if config.is_empty() {
        log::info!("Empty source config, skipped");
        return;
    }
    if get_modified_time(&path) > get_modified_time(&Config::file())
        && get_modified_time(&path) < get_exe_time()
    {
        if store_path(Config::file(), config).is_err() {
            log::info!("config written");
        }
    }
    let config2: Config2 = load_path(path2.into());
    if get_modified_time(&path2) > get_modified_time(&Config2::file()) {
        if store_path(Config2::file(), config2).is_err() {
            log::info!("config2 written");
        }
    }
}
