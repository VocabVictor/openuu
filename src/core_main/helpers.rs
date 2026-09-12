use super::*;

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

/// invoke a new connection
///
/// [Note]
/// this is for invoke new connection from dbus.
/// If it returns [`None`], then the process will terminate, and flutter gui will not be started.
/// If it returns [`Some`], then the process will continue, and flutter gui will be started.
#[cfg(feature = "flutter")]
pub(super) fn core_main_invoke_new_connection(mut args: std::env::Args) -> Option<Vec<String>> {
    let mut authority = None;
    let mut id = None;
    let mut param_array = vec![];
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--connect" | "--play" | "--file-transfer" | "--view-camera" | "--port-forward"
            | "--terminal" | "--rdp" => {
                authority = Some((&arg.to_string()[2..]).to_owned());
                id = args.next();
            }
            "--password" => {
                if let Some(password) = args.next() {
                    param_array.push(format!("password={password}"));
                }
            }
            "--relay" => {
                param_array.push(format!("relay=true"));
            }
            // inner
            "--switch_uuid" => {
                if let Some(switch_uuid) = args.next() {
                    param_array.push(format!("switch_uuid={switch_uuid}"));
                }
            }
            _ => {}
        }
    }
    let mut uni_links = Default::default();
    if let Some(authority) = authority {
        if let Some(mut id) = id {
            let app_name = crate::get_app_name();
            let ext = format!(".{}", app_name.to_lowercase());
            if id.ends_with(&ext) {
                id = id.replace(&ext, "");
            }
            let params = param_array.join("&");
            let params_flag = if params.is_empty() { "" } else { "?" };
            uni_links = format!(
                "{}{}/{}{}{}",
                crate::get_uri_prefix(),
                authority,
                id,
                params_flag,
                params
            );
        }
    }
    if uni_links.is_empty() {
        return None;
    }

    #[cfg(target_os = "linux")]
    return try_send_by_dbus(uni_links);

    #[cfg(windows)]
    {
        use winapi::um::winuser::WM_USER;
        let res = crate::platform::send_message_to_hnwd(
            &crate::platform::FLUTTER_RUNNER_WIN32_WINDOW_CLASS,
            &crate::get_app_name(),
            (WM_USER + 2) as _, // referred from unilinks desktop pub
            uni_links.as_str(),
            false,
        );
        return if res { None } else { Some(Vec::new()) };
    }
    #[cfg(target_os = "macos")]
    {
        return if let Err(_) = crate::ipc::send_url_scheme(uni_links) {
            Some(Vec::new())
        } else {
            None
        };
    }
}

#[cfg(all(target_os = "linux", feature = "flutter"))]
fn try_send_by_dbus(uni_links: String) -> Option<Vec<String>> {
    use crate::dbus::invoke_new_connection;

    match invoke_new_connection(uni_links) {
        Ok(()) => {
            return None;
        }
        Err(err) => {
            log::error!("{}", err.as_ref());
            // return Some to invoke this url by self
            return Some(Vec::new());
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn is_root() -> bool {
    #[cfg(windows)]
    {
        return crate::platform::is_elevated(None).unwrap_or_default()
            || crate::platform::is_root();
    }
    #[allow(unreachable_code)]
    crate::platform::is_root()
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn is_user_main_ipc_scope_cli_command(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("--password")
            | Some("--set-unlock-pin")
            | Some("--get-id")
            | Some("--set-id")
            | Some("--config")
            | Some("--option")
            | Some("--assign")
            | Some("--deploy")
    )
}

#[inline]
pub(super) fn is_cli_setting_change_disabled() -> bool {
    let option = keys::OPTION_ALLOW_COMMAND_LINE_SETTINGS_WHEN_SETTINGS_DISABLED;
    let allow_command_line_settings =
        config::option2bool(option, &crate::get_builtin_option(option));
    config::is_disable_settings() && !allow_command_line_settings
}

#[cfg(windows)]
pub(super) fn parse_silent_install_args(args: &[String]) -> bool {
    args.iter().skip(1).any(|arg| arg == "debug")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn user_main_ipc_scope_cli_command_matches_management_commands_only() {
        for command in [
            "--password",
            "--set-unlock-pin",
            "--get-id",
            "--set-id",
            "--config",
            "--option",
            "--assign",
            "--deploy",
        ] {
            assert!(is_user_main_ipc_scope_cli_command(&args(&[command])));
        }

        for command in [
            "--service",
            "--server",
            "--tray",
            "--cm",
            "--check-hwcodec-config",
            "--connect",
        ] {
            assert!(!is_user_main_ipc_scope_cli_command(&args(&[command])));
        }
    }
}

/// Check if the executable is a Quick Support version.
/// Note: This function must be kept in sync with `libs/portable/src/main.rs`.
#[cfg(windows)]
#[inline]
pub(super) fn is_quick_support_exe(exe: &str) -> bool {
    let exe = exe.to_lowercase();
    exe.contains("-qs-") || exe.contains("-qs.exe") || exe.contains("_qs.exe")
}
