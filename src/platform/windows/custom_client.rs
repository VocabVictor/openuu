use super::*;

#[inline]
pub fn get_custom_client_staging_dir() -> PathBuf {
    get_public_base_dir()
        .join("RustDesk")
        .join("RustDeskCustomClientStaging")
}

/// Removes the custom client staging directory.
///
/// Current behavior: intentionally a no-op (does not delete).
///
/// Rationale
/// - The staging directory only contains a small `custom.txt`, leaving it is harmless.
/// - Deleting directories under a public location (e.g., C:\\ProgramData\\RustDesk) is
///   susceptible to TOCTOU attacks if an unprivileged user can replace the path with a
///   symlink/junction between checks and deletion.
///
/// Future work:
/// - Use the files (if needed) in the installation directory instead of a public location.
///   This directory only contains a small `custom.txt` file.
/// - Pass the custom client name directly via command line
///   or environment variable during update installation. Then no staging directory is needed.
#[inline]
pub fn remove_custom_client_staging_dir(staging_dir: &Path) -> ResultType<bool> {
    if !staging_dir.exists() {
        return Ok(false);
    }

    // First explicitly removes `custom.txt` to ensure stale config is never replayed,
    // even if the subsequent directory removal fails.
    //
    // `std::fs::remove_file` on a symlink removes the symlink itself, not the target,
    // so this is safe even in a TOCTOU race.
    let custom_txt_path = staging_dir.join("custom.txt");
    if custom_txt_path.exists() {
        allow_err!(std::fs::remove_file(&custom_txt_path));
    }

    // Intentionally not deleting. See the function docs for rationale.
    log::debug!(
        "Skip deleting staging directory {:?} (intentional to avoid TOCTOU)",
        staging_dir
    );
    Ok(false)
}

// Prepare custom client update by copying staged custom.txt to current directory and loading it.
// Returns:
// 1. Ok(true) if preparation was successful or no staging directory exists.
// 2. Ok(false) if custom.txt file exists but has invalid contents or fails security checks
//    (e.g., is a symlink or has invalid contents).
// 3. Err if any unexpected error occurs during file operations.
pub fn prepare_custom_client_update() -> ResultType<bool> {
    let custom_client_staging_dir = get_custom_client_staging_dir();
    let current_exe = std::env::current_exe()?;
    let current_exe_dir = current_exe
        .parent()
        .ok_or(anyhow!("Cannot get parent directory of current exe"))?;

    let staging_dir = custom_client_staging_dir.clone();
    let clear_staging_on_exit = crate::SimpleCallOnReturn {
        b: true,
        f: Box::new(
            move || match remove_custom_client_staging_dir(&staging_dir) {
                Ok(existed) => {
                    if existed {
                        log::info!("Custom client staging directory removed successfully.");
                    }
                }
                Err(e) => {
                    log::error!(
                        "Failed to remove custom client staging directory {:?}: {}",
                        staging_dir,
                        e
                    );
                }
            },
        ),
    };

    if custom_client_staging_dir.exists() {
        let custom_txt_path = custom_client_staging_dir.join("custom.txt");
        if !custom_txt_path.exists() {
            return Ok(true);
        }

        let metadata = std::fs::symlink_metadata(&custom_txt_path)?;
        if metadata.is_symlink() {
            log::error!(
                "custom.txt is a symlink. Refusing to load custom client for security reasons."
            );
            drop(clear_staging_on_exit);
            return Ok(false);
        }
        if metadata.is_file() {
            // Copy custom.txt to current directory
            let local_custom_file_path = current_exe_dir.join("custom.txt");
            log::debug!(
                "Copying staged custom file from {:?} to {:?}",
                custom_txt_path,
                local_custom_file_path
            );

            // No need to check symlink before copying.
            // `load_custom_client()` will fail if the file is not valid.
            fs::copy(&custom_txt_path, &local_custom_file_path)?;
            log::info!("Staged custom client file copied to current directory.");

            // Load custom client
            let is_custom_file_exists =
                local_custom_file_path.exists() && local_custom_file_path.is_file();
            crate::load_custom_client();

            // Remove the copied custom.txt file
            allow_err!(fs::remove_file(&local_custom_file_path));

            // Check if loaded successfully
            if is_custom_file_exists && !crate::common::is_custom_client() {
                // The custom.txt file existed, but its contents are invalid.
                log::error!("Failed to load custom client from custom.txt.");
                drop(clear_staging_on_exit);
                // ERROR_INVALID_DATA
                return Ok(false);
            }
        } else {
            log::info!("No custom client files found in staging directory.");
        }
    } else {
        log::info!(
            "Custom client staging directory {:?} does not exist.",
            custom_client_staging_dir
        );
    }

    Ok(true)
}

pub fn get_license_from_exe_name() -> ResultType<CustomServer> {
    let mut exe = std::env::current_exe()?.to_str().unwrap_or("").to_owned();
    // if defined portable appname entry, replace original executable name with it.
    if let Ok(portable_exe) = std::env::var(PORTABLE_APPNAME_RUNTIME_ENV_KEY) {
        exe = portable_exe;
    }
    get_custom_server_from_string(&exe)
}
