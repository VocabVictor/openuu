//! First-run config import for service installs.
//!
//! The in-app installer creates a temporary service that runs
//! `--import-config <user config>` so the SYSTEM-side config starts from the
//! installing user's settings. The MSI creates the service directly, so the
//! service config began empty and the user's ID server, API server and key
//! were lost. This mirrors that import on the first `--server` start.

use super::{get_active_user_home, is_installed, is_root};
use hbb_common::config::{Config, Config2};
use std::path::PathBuf;

/// Path of the active user's `<app>.toml` when the installed service has no
/// config of its own yet; `None` in every other case.
pub fn user_config_for_first_run() -> Option<String> {
    if !is_installed() || !is_root() {
        return None;
    }
    if Config::file().exists() || Config2::file().exists() {
        return None;
    }
    let path = user_config_path(get_active_user_home()?);
    if !path.exists() {
        return None;
    }
    path.to_str().map(|s| s.to_owned())
}

fn user_config_path(home: PathBuf) -> PathBuf {
    home.join("AppData")
        .join("Roaming")
        .join(crate::get_app_name())
        .join("config")
        .join(format!("{}.toml", crate::get_app_name()))
}
