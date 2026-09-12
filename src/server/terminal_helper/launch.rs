use super::*;

/// Launch terminal helper process as the logged-in user using the provided token.
/// The helper process creates ConPTY and shell, communicating via named pipes.
/// This uses CreateProcessAsUserW directly with the user token, which works because
/// the helper process itself doesn't need ConPTY - it creates ConPTY internally.
///
/// Returns HelperProcessInfo containing the process handle and PID.

/// RAII guard for environment block cleanup.
/// Ensures DestroyEnvironmentBlock is called even if an error occurs.
pub(super) struct EnvironmentBlockGuard {
    pub(super) ptr: *mut c_void,
}

impl Drop for EnvironmentBlockGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                // Ignore result: DestroyEnvironmentBlock failure is non-critical during cleanup
                let _ = DestroyEnvironmentBlock(self.ptr);
            }
        }
    }
}

pub fn launch_terminal_helper_with_token(
    user_token: UserToken,
    input_pipe_name: &str,
    output_pipe_name: &str,
    terminal_id: i32,
    rows: u16,
    cols: u16,
) -> Result<HelperProcessInfo> {
    let exe_path =
        std::env::current_exe().map_err(|e| anyhow!("Failed to get current exe path: {}", e))?;

    // Build command line arguments (without exe path to avoid escaping issues)
    // lpApplicationName will contain the exe path separately
    let cmd_args = format!(
        "--terminal-helper {} {} {} {} {}",
        input_pipe_name, output_pipe_name, rows, cols, terminal_id
    );

    log::debug!("Launching terminal helper for terminal {}", terminal_id);

    // Convert exe path to wide string for lpApplicationName
    let exe_path_wide: Vec<u16> = OsStr::new(exe_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Command line must include exe name as first argument per Windows convention
    let cmd_line = format!("\"{}\" {}", exe_path.display(), cmd_args);
    let mut cmd_wide: Vec<u16> = OsStr::new(&cmd_line)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // Create environment block for the user with RAII cleanup
    let mut environment: *mut c_void = ptr::null_mut();
    let env_ok = unsafe {
        CreateEnvironmentBlock(
            &mut environment,
            Some(HANDLE(user_token.as_raw() as _)),
            true,
        )
    }
    .is_ok();

    // Use RAII guard to ensure cleanup even on error paths
    let _env_guard = if env_ok && !environment.is_null() {
        Some(EnvironmentBlockGuard { ptr: environment })
    } else {
        if !env_ok {
            log::warn!("Failed to create environment block, using default");
        }
        None
    };

    let creation_flags = CREATE_NO_WINDOW
        | if env_ok {
            CREATE_UNICODE_ENVIRONMENT
        } else {
            PROCESS_CREATION_FLAGS(0)
        };

    // Use lpApplicationName to pass exe path separately from command line
    // This avoids potential issues with special characters in the exe path
    let result = unsafe {
        CreateProcessAsUserW(
            Some(HANDLE(user_token.as_raw() as _)),
            PCWSTR::from_raw(exe_path_wide.as_ptr()), // lpApplicationName: exe path
            Some(PWSTR::from_raw(cmd_wide.as_mut_ptr())), // lpCommandLine: full command
            None,
            None,
            false, // Don't inherit handles
            creation_flags,
            if env_ok { Some(environment) } else { None },
            PCWSTR::null(), // Use default current directory
            &si,
            &mut pi,
        )
    };

    // Environment block cleanup is handled by _env_guard's Drop

    if let Err(e) = result {
        log::error!("CreateProcessAsUserW failed: {}", e);
        return Err(anyhow!("Failed to launch terminal helper: {}", e));
    }

    // Close thread handle - we only need the process handle for tracking
    // Ignore result: CloseHandle failure here is non-critical since process is already launched
    unsafe {
        let _ = CloseHandle(pi.hThread);
    }

    log::info!("Terminal helper launched with PID {}", pi.dwProcessId);
    // Return process info for tracking
    Ok(HelperProcessInfo {
        handle: pi.hProcess,
        pid: pi.dwProcessId,
    })
}

/// Check if a helper process is still running.
/// Returns true if the process is running, false if it has exited.
pub fn is_helper_process_running(handle: HANDLE) -> bool {
    let wait_result = unsafe { WaitForSingleObject(handle, 0) };
    // WAIT_TIMEOUT (258) means process is still running
    // WAIT_OBJECT_0 (0) means process has exited
    wait_result != WAIT_OBJECT_0
}
