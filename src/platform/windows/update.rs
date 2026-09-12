use super::*;

/// Calculate the total size of a directory in KB
/// Does not follow symlinks to prevent directory traversal attacks.
pub(super) fn get_directory_size_kb(path: &str) -> u64 {
    let mut total_size = 0u64;
    let mut stack = vec![PathBuf::from(path)];

    while let Some(current_path) = stack.pop() {
        let entries = match std::fs::read_dir(&current_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let metadata = match std::fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };

            if metadata.is_symlink() {
                continue;
            }

            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total_size = total_size.saturating_add(metadata.len());
            }
        }
    }

    total_size / 1024
}

pub fn update_me(debug: bool) -> ResultType<()> {
    let app_name = crate::get_app_name();
    let src_exe = std::env::current_exe()?.to_string_lossy().to_string();
    let (subkey, path, _, exe) = get_install_info();
    let is_installed = std::fs::metadata(&exe).is_ok();
    if !is_installed {
        bail!("{} is not installed.", &app_name);
    }
    let is_msi = is_msi_installed().ok();
    let reg_msi_key = get_reg_msi_key(&subkey, is_msi)?;

    let app_exe_name = &format!("{}.exe", &app_name);
    // NOTE: The pids below are matched by command line, which can silently come
    // back empty even while the processes are running:
    // - a 32-bit build cannot read the command line of a 64-bit process, so it
    //   shells out to `wmic` instead (#11638), and `wmic` is no longer installed
    //   by default since Windows 11 24H2;
    // - a non-elevated process cannot read the command line of an elevated one.
    // The `taskkill` in the commands below matches by image name and is not
    // affected, but `*_sessions` are then empty, so `_restore_session_guard`
    // silently restores nothing and the update leaves the user without a tray
    // icon and main window until the app is launched again. Reading the command
    // line through `NtQueryInformationProcess` instead would fix the queries for
    // every caller.
    let main_window_pids =
        crate::platform::get_pids_of_process_with_args::<_, &str>(&app_exe_name, &[]);
    let main_window_sessions = main_window_pids
        .iter()
        .map(|pid| get_session_id_of_process(pid.as_u32()))
        .flatten()
        .collect::<Vec<_>>();
    kill_process_by_pids(&app_exe_name, main_window_pids)?;
    let tray_pids = crate::platform::get_pids_of_process_with_args(&app_exe_name, &["--tray"]);
    let tray_sessions = tray_pids
        .iter()
        .map(|pid| get_session_id_of_process(pid.as_u32()))
        .flatten()
        .collect::<Vec<_>>();
    kill_process_by_pids(&app_exe_name, tray_pids)?;
    let is_service_running = is_self_service_running();

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
    let version = crate::VERSION.replace("-", ".");
    let size = get_directory_size_kb(&path);
    let build_date = crate::BUILD_DATE;
    // Use the icon in the previous installation directory if possible.
    let display_icon = get_custom_icon("", &exe).unwrap_or(exe.to_string());

    fn get_reg_cmd(
        subkey: &str,
        is_msi: Option<bool>,
        display_icon: &str,
        version: &str,
        build_date: &str,
        version_major: &str,
        version_minor: &str,
        version_build: &str,
        size: u64,
    ) -> String {
        let reg_display_icon = if is_msi.unwrap_or(false) {
            "".to_string()
        } else {
            format!(
                "reg add {} /f /v DisplayIcon /t REG_SZ /d \"{}\"",
                subkey, display_icon
            )
        };
        format!(
            "
{reg_display_icon}
reg add {subkey} /f /v DisplayVersion /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v Version /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v BuildDate /t REG_SZ /d \"{build_date}\"
reg add {subkey} /f /v VersionMajor /t REG_DWORD /d {version_major}
reg add {subkey} /f /v VersionMinor /t REG_DWORD /d {version_minor}
reg add {subkey} /f /v VersionBuild /t REG_DWORD /d {version_build}
reg add {subkey} /f /v EstimatedSize /t REG_DWORD /d {size}
        "
        )
    }

    let reg_cmd = {
        let reg_cmd_main = get_reg_cmd(
            &subkey,
            is_msi,
            &display_icon,
            &version,
            &build_date,
            &version_major,
            &version_minor,
            &version_build,
            size,
        );
        let reg_cmd_msi = if let Some(reg_msi_key) = &reg_msi_key {
            // This is best-effort: failure may leave a stale version in the Windows app list,
            // but should not interrupt the update.
            format!("reg add {reg_msi_key} /f /v DisplayVersion /t REG_SZ /d \"{version}\"")
        } else {
            "".to_owned()
        };
        format!("{}{}", reg_cmd_main, reg_cmd_msi)
    };

    let filter = format!(" /FI \"PID ne {}\"", get_current_pid());
    let restore_service_cmd = if is_service_running {
        format!("sc start {}", &app_name)
    } else {
        "".to_owned()
    };

    // We do not try to remove all files in the old version.
    // Because I don't know whether additional files will be installed here after installation, such as drivers.
    // Just copy files to the installation directory works fine.
    //if exist \"{path}\" rd /s /q \"{path}\"
    // md \"{path}\"
    //
    // We need `taskkill` because:
    // 1. There may be some other processes like `rustdesk --connect` are running.
    // 2. Sometimes, the main window and the tray icon are showing
    // while I cannot find them by `tasklist` or the methods above.
    // There's should be 4 processes running: service, server, tray and main window.
    // But only 2 processes are shown in the tasklist.
    let cmds = format!(
        "
chcp 65001
sc stop {app_name}
taskkill /F /IM {app_name}.exe{filter}
{reg_cmd}
{copy_exe}
{rename_exe}
{remove_meta_toml}
{restore_service_cmd}
{sleep}
    ",
        app_name = app_name,
        copy_exe = copy_exe_cmd(&src_exe, &exe, &path)?,
        rename_exe = rename_exe_cmd(&src_exe, &path)?,
        remove_meta_toml = remove_meta_toml_cmd(is_msi.unwrap_or(true), &path),
        sleep = if debug { "timeout 300" } else { "" },
    );

    let _restore_session_guard = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let is_root = is_root();
            if tray_sessions.is_empty() {
                log::info!("No tray process found.");
            } else {
                log::info!(
                    "Try to restore the tray process..., sessions: {:?}",
                    &tray_sessions
                );
                // When not running as root, only spawn once since run_exe_direct
                // doesn't target specific sessions.
                let mut spawned_non_root_tray = false;
                for s in tray_sessions.clone().into_iter() {
                    if s != 0 {
                        // We need to check if is_root here because if `update_me()` is called from
                        // the main window running with administrator permission,
                        // `run_exe_in_session()` will fail with error 1314 ("A required privilege is
                        // not held by the client").
                        //
                        // This issue primarily affects the MSI-installed version running in Administrator
                        // session during testing, but we check permissions here to be safe.
                        if is_root {
                            allow_err!(run_exe_in_session(&exe, vec!["--tray"], s, true));
                        } else if !spawned_non_root_tray {
                            // Only spawn once for non-root since run_exe_direct doesn't take session parameter
                            allow_err!(run_exe_direct(&exe, vec!["--tray"], false));
                            spawned_non_root_tray = true;
                        }
                    }
                }
            }
            if main_window_sessions.is_empty() {
                log::info!("No main window process found.");
            } else {
                log::info!("Try to restore the main window process...");
                std::thread::sleep(std::time::Duration::from_millis(2000));
                // When not running as root, only spawn once since run_exe_direct
                // doesn't target specific sessions.
                let mut spawned_non_root_main = false;
                for s in main_window_sessions.clone().into_iter() {
                    if s != 0 {
                        if is_root {
                            allow_err!(run_exe_in_session(&exe, vec![], s, true));
                        } else if !spawned_non_root_main {
                            // Only spawn once for non-root since run_exe_direct doesn't take session parameter
                            allow_err!(run_exe_direct(&exe, vec![], false));
                            spawned_non_root_main = true;
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }),
    };

    run_cmds(cmds, debug, "update")?;

    std::thread::sleep(std::time::Duration::from_millis(2000));
    log::info!("Update completed.");

    Ok(())
}
