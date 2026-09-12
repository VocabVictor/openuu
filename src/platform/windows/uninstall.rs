use super::*;

pub fn run_before_uninstall() -> ResultType<()> {
    run_cmds(get_before_uninstall(true), true, "before_install")
}

pub(super) fn get_before_uninstall(kill_self: bool) -> String {
    let app_name = crate::get_app_name();
    let ext = app_name.to_lowercase();
    let filter = if kill_self {
        "".to_string()
    } else {
        format!(" /FI \"PID ne {}\"", get_current_pid())
    };
    format!(
        "
    chcp 65001
    sc stop {app_name}
    sc delete {app_name}
    taskkill /F /IM {broker_exe}
    taskkill /F /IM {app_name}.exe{filter}
    reg delete HKEY_CLASSES_ROOT\\.{ext} /f
    reg delete HKEY_CLASSES_ROOT\\{ext} /f
    netsh advfirewall firewall delete rule name=\"{app_name} Service\"
    ",
        broker_exe = WIN_TOPMOST_INJECTED_PROCESS_EXE,
    )
}

/// Constructs the uninstall command string for the application.
///
/// # Parameters
/// - `kill_self`: The command will kill the process of current app name. If `true`, it will kill
///   the current process as well. If `false`, it will exclude the current process from the kill
///   command.
pub(super) fn get_uninstall(kill_self: bool) -> ResultType<String> {
    let (subkey, path, start_menu, _) = get_install_info();
    let installer_state = get_windows_installer_state(&subkey)?;
    if let Some(product_code) = get_msi_product_code(&subkey, installer_state)? {
        return Ok(build_msi_uninstall_command(&product_code));
    }
    if installer_state == Some(true) {
        bail!("MSI product code was not found in {subkey}");
    }

    let mut uninstall_cert_cmd = "".to_string();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_path) = exe.to_str() {
            uninstall_cert_cmd = format!("\"{}\" --uninstall-cert", exe_path);
        }
    }
    Ok(format!(
        "
    {before_uninstall}
    {uninstall_cert_cmd}
    reg delete {subkey} /f
    {uninstall_amyuni_idd}
    if exist \"{path}\" rd /s /q \"{path}\"
    if exist \"{start_menu}\" rd /s /q \"{start_menu}\"
    if exist \"%PUBLIC%\\Desktop\\{app_name}.lnk\" del /f /q \"%PUBLIC%\\Desktop\\{app_name}.lnk\"
    if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
    ",
        before_uninstall=get_before_uninstall(kill_self),
        uninstall_amyuni_idd=get_uninstall_amyuni_idd(),
        app_name = crate::get_app_name(),
    ))
}

pub fn uninstall_me(kill_self: bool) -> ResultType<()> {
    run_cmds(get_uninstall(kill_self)?, true, "uninstall")
}

pub fn uninstall_service(show_new_window: bool, _: bool) -> bool {
    log::info!("Uninstalling service...");
    let filter = format!(" /FI \"PID ne {}\"", get_current_pid());
    Config::set_option("stop-service".into(), "Y".into());
    let cmds = format!(
        "
    chcp 65001
    sc stop {app_name}
    sc delete {app_name}
    if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
    taskkill /F /IM {broker_exe}
    taskkill /F /IM {app_name}.exe{filter}
    ",
        app_name = crate::get_app_name(),
        broker_exe = WIN_TOPMOST_INJECTED_PROCESS_EXE,
    );
    if let Err(err) = run_cmds(cmds, false, "uninstall") {
        Config::set_option("stop-service".into(), "".into());
        log::debug!("{err}");
        return true;
    }
    run_after_run_cmds(!show_new_window);
    std::process::exit(0);
}

pub(super) fn get_install_service_commands(path: &str, exe: &str) -> ResultType<String> {
    let app_name = crate::get_app_name();
    for value in [path, exe] {
        validate_install_value(value)?;
    }
    let config_path = Config::file();
    validate_install_value(
        config_path
            .to_str()
            .ok_or_else(|| anyhow!("Configuration path is not valid Unicode"))?,
    )?;
    let shortcut_icon_location = get_custom_icon(path, exe);
    if let Some(icon) = shortcut_icon_location.as_deref() {
        validate_install_value(icon)?;
    }
    let tray_shortcut_commands =
        embedded_tray_shortcut_commands(&app_name, exe, shortcut_icon_location.as_deref())?;
    let filter = format!(" /FI \"PID ne {}\"", get_current_pid());
    Ok(format!(
        "
chcp 65001
taskkill /F /IM {app_name}.exe{filter}
{tray_shortcut_commands}
copy /Y \"%RUSTDESK_OUTPUT_DIR%\\{app_name} Tray.lnk\" \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\\"
{import_config}
{create_service}
    ",
        import_config = get_import_config(exe),
        create_service = get_create_service(exe),
    ))
}

pub fn install_service() -> bool {
    log::info!("Installing service...");
    let _installing = crate::platform::InstallingService::new();
    let (_, path, _, exe) = get_install_info();
    Config::set_option("stop-service".into(), "".into());
    let cmds = match get_install_service_commands(&path, &exe) {
        Ok(cmds) => cmds,
        Err(err) => {
            Config::set_option("stop-service".into(), "Y".into());
            log::error!("Failed to prepare service installation: {err}");
            return true;
        }
    };
    crate::ipc::EXIT_RECV_CLOSE.store(false, Ordering::Relaxed);
    if let Err(err) = run_cmds(cmds, false, "install") {
        Config::set_option("stop-service".into(), "Y".into());
        crate::ipc::EXIT_RECV_CLOSE.store(true, Ordering::Relaxed);
        log::debug!("{err}");
        return true;
    }
    run_after_run_cmds(false);
    std::process::exit(0);
}
