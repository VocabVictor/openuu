use super::*;

// Double confirm the process name
pub(super) fn kill_process_by_pids(name: &str, pids: Vec<Pid>) -> ResultType<()> {
    let name = name.to_lowercase();
    let s = System::new_all();
    // No need to check all names of `pids` first, and kill them then.
    // It's rare case that they're not matched.
    for pid in pids {
        if let Some(process) = s.process(pid) {
            if process.name().to_lowercase() != name {
                bail!("Failed to kill the process, the names are mismatched.");
            }
            if !process.kill() {
                bail!("Failed to kill the process");
            }
        } else {
            bail!("Failed to kill the process, the pid is not found");
        }
    }
    Ok(())
}

pub fn handle_custom_client_staging_dir_before_update(
    custom_client_staging_dir: &PathBuf,
) -> ResultType<()> {
    let Some(current_exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    else {
        bail!("Failed to get current exe directory");
    };

    // Clean up existing staging directory
    if custom_client_staging_dir.exists() {
        log::debug!(
            "Removing existing custom client staging directory: {:?}",
            custom_client_staging_dir
        );
        if let Err(e) = remove_custom_client_staging_dir(custom_client_staging_dir) {
            bail!(
                "Failed to remove existing custom client staging directory {:?}: {}",
                custom_client_staging_dir,
                e
            );
        }
    }

    let src_path = current_exe_dir.join("custom.txt");
    if src_path.exists() {
        // Verify that custom.txt is not a symlink before copying
        let metadata = match std::fs::symlink_metadata(&src_path) {
            Ok(m) => m,
            Err(e) => {
                bail!(
                    "Failed to read metadata for custom.txt at {:?}: {}",
                    src_path,
                    e
                );
            }
        };

        if metadata.is_symlink() {
            allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
            bail!(
                "custom.txt at {:?} is a symlink, refusing to stage for security reasons.",
                src_path
            );
        }

        if metadata.is_file() {
            if !custom_client_staging_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(custom_client_staging_dir) {
                    bail!("Failed to create parent directory {:?} when staging custom client files: {}", custom_client_staging_dir, e);
                }
            }
            let dst_path = custom_client_staging_dir.join("custom.txt");
            if let Err(e) = std::fs::copy(&src_path, &dst_path) {
                allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
                bail!(
                    "Failed to copy custom txt from {:?} to {:?}: {}",
                    src_path,
                    dst_path,
                    e
                );
            }
        } else {
            log::warn!(
                "custom.txt at {:?} is not a regular file, skipping.",
                src_path
            );
        }
    } else {
        log::info!("No custom txt found to stage for update.");
    }

    Ok(())
}

// Used for auto update and manual update in the main window.
pub fn update_to(file: &str) -> ResultType<()> {
    if file.ends_with(".exe") {
        let custom_client_staging_dir = get_custom_client_staging_dir();
        if crate::is_custom_client() {
            handle_custom_client_staging_dir_before_update(&custom_client_staging_dir)?;
        } else {
            // Clean up any residual staging directory from previous custom client
            allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
        }
        if !run_uac(file, "--update")? {
            bail!(
                "Failed to run the update exe with UAC, error: {:?}",
                std::io::Error::last_os_error()
            );
        }
    } else if file.ends_with(".msi") {
        if let Err(e) = update_me_msi(file, false) {
            bail!("Failed to run the update msi: {}", e);
        }
    } else {
        // unreachable!()
        bail!("Unsupported update file format: {}", file);
    }
    Ok(())
}

pub(super) fn get_import_config(exe: &str) -> String {
    if config::is_outgoing_only() {
        return "".to_string();
    }
    let exe = escape_nested_cmd_ampersands(exe);
    let config_path = Config::file();
    let config_path = escape_nested_cmd_ampersands(config_path.to_str().unwrap_or(""));
    format!("
sc stop {app_name}
sc delete {app_name}
sc create {app_name} binpath= \"\\\"{exe}\\\" --import-config \\\"{config_path}\\\"\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
sc stop {app_name}
sc delete {app_name}
",
    app_name = crate::get_app_name(),
)
}

pub(super) fn get_create_service(exe: &str) -> String {
    if config::is_outgoing_only() {
        return "".to_string();
    }
    let stop = Config::get_option("stop-service") == "Y";
    if stop {
        format!("
if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
", app_name = crate::get_app_name())
    } else {
        let exe = escape_nested_cmd_ampersands(exe);
        format!("
sc create {app_name} binpath= \"\\\"{exe}\\\" --service\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
",
    app_name = crate::get_app_name())
    }
}

pub(super) fn run_after_run_cmds(silent: bool) {
    let (_, _, _, exe) = get_install_info();
    if !silent {
        log::debug!("Spawn new window");
        allow_err!(std::process::Command::new("cmd")
            .args(&["/c", "timeout", "/t", "2", "&", &format!("{exe}")])
            .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
            .spawn());
    }
    if Config::get_option("stop-service") != "Y" {
        allow_err!(std::process::Command::new(&exe).arg("--tray").spawn());
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
}

#[inline]
pub fn try_remove_temp_update_files() {
    let temp_dir = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp_dir) else {
        log::debug!("Failed to read temp directory: {:?}", temp_dir);
        return;
    };

    let one_hour = std::time::Duration::from_secs(60 * 60);
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                // Match files like rustdesk-*.msi or rustdesk-*.exe
                if file_name.starts_with("rustdesk-")
                    && (file_name.ends_with(".msi") || file_name.ends_with(".exe"))
                {
                    // Skip files modified within the last hour to avoid deleting files being downloaded
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed < one_hour {
                                    continue;
                                }
                            }
                        }
                    }
                    if let Err(e) = std::fs::remove_file(&path) {
                        log::debug!("Failed to remove temp update file {:?}: {}", path, e);
                    } else {
                        log::info!("Removed temp update file: {:?}", path);
                    }
                }
            }
        }
    }
}

#[inline]
pub fn try_kill_broker() {
    allow_err!(std::process::Command::new("cmd")
        .arg("/c")
        .arg(&format!(
            "taskkill /F /IM {}",
            WIN_TOPMOST_INJECTED_PROCESS_EXE
        ))
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
        .spawn());
}
