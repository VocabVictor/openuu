use super::*;

pub(super) fn get_after_install(
    exe: &str,
    reg_value_start_menu_shortcuts: Option<String>,
    reg_value_desktop_shortcuts: Option<String>,
) -> String {
    let app_name = crate::get_app_name();
    let ext = app_name.to_lowercase();
    let nested_exe = escape_nested_cmd_ampersands(exe);

    // reg delete HKEY_CURRENT_USER\Software\Classes for
    // https://github.com/rustdesk/rustdesk/commit/f4bdfb6936ae4804fc8ab1cf560db192622ad01a
    // and https://github.com/leanflutter/uni_links_desktop/blob/1b72b0226cec9943ca8a84e244c149773f384e46/lib/src/protocol_registrar_impl_windows.dart#L30
    let hcu = RegKey::predef(HKEY_CURRENT_USER);
    hcu.delete_subkey_all(format!("Software\\Classes\\{}", exe))
        .ok();

    let desktop_shortcuts = reg_value_desktop_shortcuts
        .map(|v| {
            format!("reg add HKEY_CLASSES_ROOT\\.{ext} /f /v {REG_NAME_INSTALL_DESKTOPSHORTCUTS} /t REG_SZ /d \"{v}\"")
        })
        .unwrap_or_default();
    let start_menu_shortcuts = reg_value_start_menu_shortcuts
        .map(|v| {
            format!(
                "reg add HKEY_CLASSES_ROOT\\.{ext} /f /v {REG_NAME_INSTALL_STARTMENUSHORTCUTS} /t REG_SZ /d \"{v}\""
            )
        })
        .unwrap_or_default();

    format!("
    chcp 65001
    reg add HKEY_CLASSES_ROOT\\.{ext} /f
    {desktop_shortcuts}
    {start_menu_shortcuts}
    reg add HKEY_CLASSES_ROOT\\.{ext}\\DefaultIcon /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\DefaultIcon /f /ve /t REG_SZ  /d \"\\\"{nested_exe}\\\",0\"
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open\\command /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open\\command /f /ve /t REG_SZ /d \"\\\"{nested_exe}\\\" --play \\\"%%1\\\"\"
    reg add HKEY_CLASSES_ROOT\\{ext} /f
    reg add HKEY_CLASSES_ROOT\\{ext} /f /v \"URL Protocol\" /t REG_SZ /d \"\"
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell /f
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell\\open /f
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell\\open\\command /f
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell\\open\\command /f /ve /t REG_SZ /d \"\\\"{nested_exe}\\\" \\\"%%1\\\"\"
    netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=out action=allow program=\"{exe}\" enable=yes
    netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=in action=allow program=\"{exe}\" enable=yes
    {create_service}
    reg add HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System /f /v SoftwareSASGeneration /t REG_DWORD /d 1
    ", create_service=get_create_service(&exe))
}

pub fn install_me(options: &str, path: String, silent: bool, debug: bool) -> ResultType<()> {
    // MSI and EXE installations use different registry layouts, so MSI-to-EXE upgrades are not supported.
    let (installed_subkey, _, _, _) = get_install_info();
    if get_windows_installer_state(&installed_subkey)? == Some(true) {
        bail!("Cannot install the EXE package over an existing MSI installation");
    }
    let uninstall_str = get_uninstall(false)?;
    let mut path = path.trim_end_matches('\\').to_owned();
    let (subkey, _path, start_menu, exe) = get_default_install_info();
    let mut exe = exe;
    if path.is_empty() {
        path = _path;
    } else {
        exe = exe.replace(&_path, &path);
    }
    let mut version_major = "0";
    let mut version_minor = "0";
    let mut version_build = "0";
    let versions: Vec<&str> = crate::VERSION.split(".").collect();
    if versions.len() > 0 {
        version_major = versions[0];
    }
    if versions.len() > 1 {
        version_minor = versions[1];
    }
    if versions.len() > 2 {
        version_build = versions[2];
    }
    let app_name = crate::get_app_name();

    let current_exe = std::env::current_exe()?;
    let cur_exe = current_exe
        .to_str()
        .ok_or_else(|| anyhow!("Current executable path is not valid Unicode"))?
        .to_owned();
    for value in [&path, &exe, &cur_exe] {
        validate_install_value(value)?;
    }
    let config_path = Config::file();
    validate_install_value(
        config_path
            .to_str()
            .ok_or_else(|| anyhow!("Configuration path is not valid Unicode"))?,
    )?;
    let shortcut_icon_location = get_custom_icon(&path, &cur_exe);
    if let Some(icon) = shortcut_icon_location.as_deref() {
        validate_install_value(icon)?;
    }
    // The elevated runner expands this to `%~f0.dir`, beside its protected copy.
    // Do not stage privileged shortcut artifacts in the user-writable `%TEMP%`.
    let tmp_path = "%RUSTDESK_OUTPUT_DIR%".to_owned();
    let mk_shortcut_commands = embedded_shortcut_commands(
        shortcut_bytes(&exe, None, shortcut_icon_location.as_deref())?,
        &format!("{app_name}.lnk"),
        "mk_shortcut",
    );
    let uninstall_shortcut_commands = embedded_shortcut_commands(
        shortcut_bytes(&exe, Some("--uninstall"), Some("msiexec.exe"))?,
        &format!("Uninstall {app_name}.lnk"),
        "uninstall_shortcut",
    );
    let tray_shortcut_commands =
        embedded_tray_shortcut_commands(&app_name, &exe, shortcut_icon_location.as_deref())?;
    let mut reg_value_desktop_shortcuts = "0".to_owned();
    let mut reg_value_start_menu_shortcuts = "0".to_owned();
    let mut shortcuts = Default::default();
    if options.contains("desktopicon") {
        shortcuts = format!(
            "copy /Y \"{}\\{}.lnk\" \"%PUBLIC%\\Desktop\\\"",
            tmp_path,
            crate::get_app_name()
        );
        reg_value_desktop_shortcuts = "1".to_owned();
    }
    if options.contains("startmenu") {
        shortcuts = format!(
            "{shortcuts}
md \"{start_menu}\"
copy /Y \"{tmp_path}\\{app_name}.lnk\" \"{start_menu}\\\"
copy /Y \"{tmp_path}\\Uninstall {app_name}.lnk\" \"{start_menu}\\\"
     "
        );
        reg_value_start_menu_shortcuts = "1".to_owned();
    }

    let meta = std::fs::symlink_metadata(&current_exe)?;
    let mut size = meta.len() / 1024;
    if let Some(parent_dir) = current_exe.parent() {
        if let Some(d) = parent_dir.to_str() {
            size = get_directory_size_kb(d);
        }
    }
    // https://docs.microsoft.com/zh-cn/windows/win32/msi/uninstall-registry-key?redirectedfrom=MSDNa
    // https://www.windowscentral.com/how-edit-registry-using-command-prompt-windows-10
    // https://www.tenforums.com/tutorials/70903-add-remove-allowed-apps-through-windows-firewall-windows-10-a.html
    // Note: without if exist, the bat may exit in advance on some Windows7 https://github.com/rustdesk/rustdesk/issues/895
    let dels = format!(
        "
if exist \"{tmp_path}\\{app_name}.lnk\" del /f /q \"{tmp_path}\\{app_name}.lnk\"
if exist \"{tmp_path}\\Uninstall {app_name}.lnk\" del /f /q \"{tmp_path}\\Uninstall {app_name}.lnk\"
if exist \"{tmp_path}\\{app_name} Tray.lnk\" del /f /q \"{tmp_path}\\{app_name} Tray.lnk\"
        "
    );
    let src_exe = cur_exe.clone();

    // potential bug here: if run_cmd cancelled, but config file is changed.
    if let Some(lic) = get_license() {
        Config::set_option("key".into(), lic.key);
        Config::set_option("custom-rendezvous-server".into(), lic.host);
        Config::set_option("api-server".into(), lic.api);
    }

    let tray_shortcuts = if config::is_outgoing_only() {
        "".to_owned()
    } else {
        format!("
{tray_shortcut_commands}
copy /Y \"{tmp_path}\\{app_name} Tray.lnk\" \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\\"
")
    };

    // Remember to check if `update_me` need to be changed if changing the `cmds`.
    // No need to merge the existing dup code, because the code in these two functions are too critical.
    // New code should be written in a common function.
    let cmds = format!(
        "
{uninstall_str}
chcp 65001
md \"{path}\"
{copy_exe}
reg add {subkey} /f
reg add {subkey} /f /v DisplayIcon /t REG_SZ /d \"{display_icon}\"
reg add {subkey} /f /v DisplayName /t REG_SZ /d \"{app_name}\"
reg add {subkey} /f /v DisplayVersion /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v Version /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v BuildDate /t REG_SZ /d \"{build_date}\"
reg add {subkey} /f /v InstallLocation /t REG_SZ /d \"{path}\"
reg add {subkey} /f /v Publisher /t REG_SZ /d \"{app_name}\"
reg add {subkey} /f /v VersionMajor /t REG_DWORD /d {version_major}
reg add {subkey} /f /v VersionMinor /t REG_DWORD /d {version_minor}
reg add {subkey} /f /v VersionBuild /t REG_DWORD /d {version_build}
reg add {subkey} /f /v UninstallString /t REG_SZ /d \"\\\"{nested_exe}\\\" --uninstall\"
reg add {subkey} /f /v EstimatedSize /t REG_DWORD /d {size}
reg add {subkey} /f /v WindowsInstaller /t REG_DWORD /d 0
{mk_shortcut_commands}
{uninstall_shortcut_commands}
{tray_shortcuts}
{shortcuts}
copy /Y \"{tmp_path}\\Uninstall {app_name}.lnk\" \"{path}\\\"
{dels}
{import_config}
{after_install}
{sleep}
    ",
        display_icon = shortcut_icon_location.as_deref().unwrap_or(exe.as_str()),
        nested_exe = escape_nested_cmd_ampersands(&exe),
        version = crate::VERSION.replace("-", "."),
        build_date = crate::BUILD_DATE,
        after_install = get_after_install(
            &exe,
            Some(reg_value_start_menu_shortcuts),
            Some(reg_value_desktop_shortcuts),
        ),
        sleep = if debug { "timeout 300" } else { "" },
        dels = if debug { "" } else { &dels },
        copy_exe = copy_exe_cmd(&src_exe, &exe, &path)?,
        import_config = get_import_config(&exe),
    );
    run_cmds(cmds, debug, "install")?;
    run_after_run_cmds(silent);
    Ok(())
}

pub fn run_after_install() -> ResultType<()> {
    let (_, _, _, exe) = get_install_info();
    run_cmds(
        get_after_install(&exe, None, None),
        true,
        "after_install",
    )
}
