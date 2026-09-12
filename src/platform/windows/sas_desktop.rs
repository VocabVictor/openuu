use super::*;

// https://docs.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas
// https://www.cnblogs.com/doutu/p/4892726.html
pub fn send_sas() {
    #[link(name = "sas")]
    extern "system" {
        pub fn SendSAS(AsUser: BOOL);
    }
    unsafe {
        log::info!("SAS received");

        // Check and temporarily set SoftwareSASGeneration if needed
        let mut original_value: Option<u32> = None;
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        if let Ok(policy_key) = hklm.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
            KEY_READ | KEY_WRITE,
        ) {
            // Read current value
            match policy_key.get_value::<u32, _>("SoftwareSASGeneration") {
                Ok(value) => {
                    /*
                    - 0 = None (disabled)
                    - 1 = Services
                    - 2 = Ease of Access applications
                    - 3 = Services and Ease of Access applications (Both)
                                      */
                    if value != 1 && value != 3 {
                        original_value = Some(value);
                        log::info!("SoftwareSASGeneration is {}, setting to 1", value);
                        // Set to 1 for SendSAS to work
                        if let Err(e) = policy_key.set_value("SoftwareSASGeneration", &1u32) {
                            log::error!("Failed to set SoftwareSASGeneration: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::info!(
                        "SoftwareSASGeneration not found or error reading: {}, setting to 1",
                        e
                    );
                    original_value = Some(0); // Mark that we need to restore (delete) it
                                              // Create and set to 1
                    if let Err(e) = policy_key.set_value("SoftwareSASGeneration", &1u32) {
                        log::error!("Failed to set SoftwareSASGeneration: {}", e);
                    }
                }
            }
        } else {
            log::error!("Failed to open registry key for SoftwareSASGeneration");
        }

        // Send SAS
        SendSAS(FALSE);

        // Restore original value if we changed it
        if let Some(original) = original_value {
            if let Ok(policy_key) = hklm.open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System",
                KEY_WRITE,
            ) {
                if original == 0 {
                    // It didn't exist before, delete it
                    if let Err(e) = policy_key.delete_value("SoftwareSASGeneration") {
                        log::error!("Failed to delete SoftwareSASGeneration: {}", e);
                    } else {
                        log::info!("Deleted SoftwareSASGeneration (restored to original state)");
                    }
                } else {
                    // Restore the original value
                    if let Err(e) = policy_key.set_value("SoftwareSASGeneration", &original) {
                        log::error!(
                            "Failed to restore SoftwareSASGeneration to {}: {}",
                            original,
                            e
                        );
                    } else {
                        log::info!("Restored SoftwareSASGeneration to {}", original);
                    }
                }
            }
        }
    }
}

lazy_static::lazy_static! {
    static ref SUPPRESS: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
}

pub fn desktop_changed() -> bool {
    unsafe { inputDesktopSelected() == FALSE }
}

pub fn try_change_desktop() -> bool {
    unsafe {
        if inputDesktopSelected() == FALSE {
            let res = selectInputDesktop() == TRUE;
            if !res {
                let mut s = SUPPRESS.lock().unwrap();
                if s.elapsed() > std::time::Duration::from_secs(3) {
                    log::error!("Failed to switch desktop: {}", io::Error::last_os_error());
                    *s = Instant::now();
                }
            } else {
                log::info!("Desktop switched");
            }
            return res;
        }
    }
    return false;
}
