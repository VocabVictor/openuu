use super::*;

pub fn elevate_or_run_as_system(is_setup: bool, is_elevate: bool, is_run_as_system: bool) {
    // avoid possible run recursively due to failed run.
    log::info!(
        "elevate: {} -> {:?}, run_as_system: {} -> {}",
        is_elevate,
        is_elevated(None),
        is_run_as_system,
        crate::username(),
    );
    let mut arg_elevate = if is_setup {
        "--noinstall --elevate"
    } else {
        "--elevate"
    }
    .to_owned();
    let mut arg_run_as_system = if is_setup {
        "--noinstall --run-as-system"
    } else {
        "--run-as-system"
    }
    .to_owned();
    let shmem_name_from_args = crate::portable_service::portable_service_shmem_name_from_args();
    if shmem_name_from_args.is_none() && crate::portable_service::has_portable_service_shmem_arg() {
        log::error!("Invalid portable service shared memory argument, aborting elevation flow");
        // This is a malformed bootstrap argument in a privilege-sensitive path.
        // Keep fail-closed process termination here to avoid continuing elevation
        // with inconsistent shared-memory contract.
        std::process::exit(1);
    }
    if let Some(shmem_name) = shmem_name_from_args {
        let shmem_arg = crate::portable_service::portable_service_shmem_arg(&shmem_name);
        arg_elevate.push(' ');
        arg_elevate.push_str(&shmem_arg);
        arg_run_as_system.push(' ');
        arg_run_as_system.push_str(&shmem_arg);
    }
    if is_root() {
        if is_run_as_system {
            log::info!("run portable service");
            crate::portable_service::server::run_portable_service();
        }
    } else {
        match is_elevated(None) {
            Ok(elevated) => {
                if elevated {
                    if !is_run_as_system {
                        if run_as_system(arg_run_as_system.as_str()).is_ok() {
                            std::process::exit(0);
                        } else {
                            log::error!(
                                "Failed to run as system, error {}",
                                io::Error::last_os_error()
                            );
                        }
                    }
                } else {
                    if !is_elevate {
                        if let Ok(true) = elevate(arg_elevate.as_str()) {
                            std::process::exit(0);
                        } else {
                            log::error!("Failed to elevate, error {}", io::Error::last_os_error());
                        }
                    }
                }
            }
            Err(_) => log::error!(
                "Failed to get elevation status, error {}",
                io::Error::last_os_error()
            ),
        }
    }
}

pub fn is_elevated(process_id: Option<DWORD>) -> ResultType<bool> {
    use base::platform::windows::RAIIHandle;
    unsafe {
        let handle: HANDLE = match process_id {
            Some(process_id) => OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process_id),
            None => GetCurrentProcess(),
        };
        if handle == NULL {
            bail!(
                "Failed to open process, error {}",
                io::Error::last_os_error()
            )
        }
        let _handle = RAIIHandle(handle);
        let mut token: HANDLE = mem::zeroed();
        if OpenProcessToken(handle, TOKEN_QUERY, &mut token) == FALSE {
            bail!(
                "Failed to open process token, error {}",
                io::Error::last_os_error()
            )
        }
        let _token = RAIIHandle(token);
        let mut token_elevation: TOKEN_ELEVATION = mem::zeroed();
        let mut size: DWORD = 0;
        if GetTokenInformation(
            token,
            TokenElevation,
            (&mut token_elevation) as *mut _ as *mut c_void,
            mem::size_of::<TOKEN_ELEVATION>() as _,
            &mut size,
        ) == FALSE
        {
            bail!(
                "Failed to get token information, error {}",
                io::Error::last_os_error()
            )
        }

        Ok(token_elevation.TokenIsElevated != 0)
    }
}

#[inline]
pub(super) unsafe fn read_token_user_buffer(token: WinHANDLE, subject: &str) -> ResultType<Vec<u8>> {
    let mut token_user_size = 0u32;
    let get_info_result = WinGetTokenInformation(token, TokenUser, None, 0, &mut token_user_size);
    match get_info_result {
        Ok(()) => {
            if token_user_size == 0 {
                bail!(
                    "Failed to get {} token user size: unexpected zero buffer size",
                    subject
                );
            }
        }
        Err(e) => {
            // Allow expected size-probe failures if Windows still returns required size.
            let is_insufficient_buffer =
                e.code() == windows::core::HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER as u32);
            let is_bad_length =
                e.code() == windows::core::HRESULT::from_win32(ERROR_BAD_LENGTH as u32);
            if (!is_insufficient_buffer && !is_bad_length) || token_user_size == 0 {
                bail!("Failed to get {} token user size: {}", subject, e);
            }
        }
    }

    let mut buffer = vec![0u8; token_user_size as usize];
    WinGetTokenInformation(
        token,
        TokenUser,
        Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
        token_user_size,
        &mut token_user_size,
    )
    .map_err(|e| anyhow!("Failed to get {} token user: {}", subject, e))?;

    let min_size = std::mem::size_of::<TOKEN_USER>();
    if buffer.len() < min_size {
        bail!(
            "Failed to parse {} token user: buffer too small (got {}, need >= {})",
            subject,
            buffer.len(),
            min_size
        );
    }
    Ok(buffer)
}

/// Similar to `is_root()` / `is_local_system()` but for an arbitrary process.
///
/// Returns `true` if the target process is running as LocalSystem (SID: S-1-5-18).
///
/// TODO: After a few releases of real-world validation, consider replacing
/// the legacy `is_local_system()` with this implementation.
pub fn is_process_running_as_system(process_id: DWORD) -> ResultType<bool> {
    unsafe {
        let process = WinOpenProcess(WIN_PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .map_err(|e| anyhow!("Failed to open process {}: {}", process_id, e))?;

        let mut token = WinHANDLE::default();
        let result = (|| -> ResultType<bool> {
            WinOpenProcessToken(process, WIN_TOKEN_QUERY, &mut token)
                .map_err(|e| anyhow!("Failed to open process {} token: {}", process_id, e))?;

            let token_subject = format!("process {}", process_id);
            let buffer = read_token_user_buffer(token, token_subject.as_str())?;
            let token_user: TOKEN_USER =
                std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_USER);
            Ok(IsWellKnownSid(token_user.User.Sid, WinLocalSystemSid).as_bool())
        })();

        if !token.is_invalid() {
            let _ = WinCloseHandle(token);
        }
        let _ = WinCloseHandle(process);
        result
    }
}

pub fn get_process_executable_path(process_id: DWORD) -> ResultType<PathBuf> {
    const PROCESS_IMAGE_PATH_BUFFER_LEN: usize = 32 * 1024;
    unsafe {
        let process = WinOpenProcess(WIN_PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
            .map_err(|e| anyhow!("Failed to open process {}: {}", process_id, e))?;

        let result = (|| -> ResultType<PathBuf> {
            let mut buffer = vec![0u16; PROCESS_IMAGE_PATH_BUFFER_LEN];
            let mut length = PROCESS_IMAGE_PATH_BUFFER_LEN as u32;
            WinQueryFullProcessImageNameW(
                process,
                windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut length,
            )
            .map_err(|e| anyhow!("Failed to query process {} image path: {}", process_id, e))?;
            if length == 0 {
                bail!(
                    "Failed to query process {} image path: empty result",
                    process_id
                );
            }
            buffer.truncate(length as usize);
            Ok(PathBuf::from(OsString::from_wide(&buffer)))
        })();

        let _ = WinCloseHandle(process);
        result
    }
}

pub fn is_foreground_window_elevated() -> ResultType<bool> {
    unsafe {
        let mut process_id: DWORD = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), &mut process_id);
        if process_id == 0 {
            bail!(
                "Failed to get processId, error {}",
                io::Error::last_os_error()
            )
        }
        is_elevated(Some(process_id))
    }
}

pub(super) fn get_current_pid() -> u32 {
    unsafe { GetCurrentProcessId() }
}

pub fn get_double_click_time() -> u32 {
    unsafe { GetDoubleClickTime() }
}
