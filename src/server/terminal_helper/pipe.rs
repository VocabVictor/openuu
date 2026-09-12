use super::*;

/// Create a named pipe with a restricted DACL.
/// Only SYSTEM and the specified user can access the pipe.
///
/// # Arguments
/// * `pipe_name` - The name of the pipe to create
/// * `for_input` - True if service writes to this pipe (helper reads), false otherwise
/// * `user_token` - Required user token for creating restricted DACL
///
/// # Security
///
/// The restricted DACL limits pipe access to:
/// - SYSTEM account (the service)
/// - The specific user whose token was provided (the helper process)
///
/// This function requires a valid user_token and will fail if DACL creation fails,
/// rather than falling back to a less secure NULL DACL.
pub fn create_named_pipe_server(
    pipe_name: &str,
    for_input: bool,
    user_token: UserToken,
) -> Result<HANDLE> {
    // SECURITY_DESCRIPTOR minimum length is 40 bytes on x64.
    pub(super) const SD_BUFFER_SIZE: usize = 64;
    pub(super) const _: () = assert!(
        SD_BUFFER_SIZE >= 40,
        "SD_BUFFER_SIZE must be at least 40 bytes for SECURITY_DESCRIPTOR"
    );

    let mut sd_buffer = [0u8; SD_BUFFER_SIZE];
    let sd_ptr = PSECURITY_DESCRIPTOR(sd_buffer.as_mut_ptr() as *mut c_void);

    // Initialize security descriptor
    unsafe {
        InitializeSecurityDescriptor(sd_ptr, 1)
            .map_err(|e| anyhow!("Failed to initialize security descriptor: {}", e))?;
    }

    // Create restricted DACL - fail if this doesn't work (no NULL DACL fallback)
    let user_sid = get_user_sid_from_token(user_token)
        .context("Failed to get user SID from token for pipe DACL")?;
    let acl_ptr =
        create_restricted_dacl(&user_sid).context("Failed to create restricted DACL for pipe")?;

    log::debug!("Created restricted DACL for pipe: {}", pipe_name);

    // Set DACL on security descriptor
    unsafe {
        SetSecurityDescriptorDacl(sd_ptr, true, Some(acl_ptr as *const _ as *const _), false)
            .map_err(|e| {
                // Clean up ACL on error (ignore result - cleanup is best-effort, original error takes precedence)
                let _ = LocalFree(Some(HLOCAL(acl_ptr)));
                anyhow!("Failed to set restricted DACL: {}", e)
            })?;
    }

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_buffer.as_mut_ptr() as *mut c_void,
        bInheritHandle: false.into(),
    };

    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let access_mode = if for_input {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED.0)
    } else {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_OUTBOUND | FILE_FLAG_OVERLAPPED.0)
    };

    log::debug!(
        "Creating named pipe: {} (for_input={}, restricted_dacl=true)",
        pipe_name,
        for_input
    );

    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            access_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1, // max instances
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            PIPE_DEFAULT_TIMEOUT_MS,
            Some(&sa),
        )
    };

    // Clean up ACL after pipe creation (security descriptor has been applied)
    // Ignore result: LocalFree failure is non-critical since the pipe is already created
    unsafe {
        let _ = LocalFree(Some(HLOCAL(acl_ptr)));
    }

    if handle == INVALID_HANDLE_VALUE {
        return Err(anyhow!(
            "Failed to create named pipe {}: {}",
            pipe_name,
            std::io::Error::last_os_error()
        ));
    }

    log::debug!("Named pipe created: {}", pipe_name);
    Ok(handle)
}

/// Wait for client to connect to named pipe with timeout.
///
/// # Ownership
/// This function **takes ownership** of the `pipe_handle` via OwnedHandle:
/// - On success: the handle is extracted and wrapped in a `File`.
/// - On failure: the handle is automatically closed when OwnedHandle drops.
pub fn wait_for_pipe_connection(
    pipe_handle: OwnedHandle,
    pipe_name: &str,
    timeout_ms: u32,
) -> Result<File> {
    log::debug!("Waiting for pipe connection: {}", pipe_name);

    // Create an event for overlapped I/O (also wrapped in OwnedHandle for RAII)
    let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
        .map_err(|e| anyhow!("Failed to create event for pipe connection: {}", e))?;
    let event_handle = OwnedHandle::new(event);

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event_handle.as_raw();

    let result = unsafe { ConnectNamedPipe(pipe_handle.as_raw(), Some(&mut overlapped)) };
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        let err_code = err.raw_os_error().unwrap_or(0);

        // ERROR_PIPE_CONNECTED means client already connected, which is OK
        if err_code == ERROR_PIPE_CONNECTED.0 as i32 {
            log::debug!("Pipe already connected: {}", pipe_name);
            return Ok(unsafe { File::from_raw_handle(pipe_handle.into_raw().0 as RawHandle) });
        }

        // ERROR_IO_PENDING means we need to wait
        if err_code == ERROR_IO_PENDING.0 as i32 {
            log::debug!("Pipe connection pending, waiting with timeout...");
            let wait_result = unsafe { WaitForSingleObject(event_handle.as_raw(), timeout_ms) };

            if wait_result != WAIT_OBJECT_0 {
                log::error!("Timeout waiting for pipe connection: {}", pipe_name);
                return Err(anyhow!(
                    "Timeout waiting for pipe connection: {}",
                    pipe_name
                ));
            }

            // Check if connection was successful
            let mut bytes_transferred = 0u32;
            let overlapped_result = unsafe {
                GetOverlappedResult(
                    pipe_handle.as_raw(),
                    &overlapped,
                    &mut bytes_transferred,
                    false,
                )
            };
            if overlapped_result.is_err() {
                let err = std::io::Error::last_os_error();
                log::error!("Failed to complete pipe connection {}: {}", pipe_name, err);
                return Err(anyhow!(
                    "Failed to complete pipe connection {}: {}",
                    pipe_name,
                    err
                ));
            }

            log::debug!("Pipe connected: {}", pipe_name);
        } else {
            log::error!("Failed to connect named pipe {}: {}", pipe_name, err);
            return Err(anyhow!(
                "Failed to connect named pipe {}: {}",
                pipe_name,
                err
            ));
        }
    } else {
        log::debug!("Pipe connected immediately: {}", pipe_name);
    }

    // Success: transfer pipe ownership to File, event_handle drops
    Ok(unsafe { File::from_raw_handle(pipe_handle.into_raw().0 as RawHandle) })
}

/// Open a named pipe as a client.
/// `for_read`: true for reading (input pipe), false for writing (output pipe).
pub(super) fn open_pipe(pipe_name: &str, for_read: bool) -> Result<File> {
    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let access = if for_read {
        FILE_GENERIC_READ.0
    } else {
        FILE_GENERIC_WRITE.0
    };

    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };

    match handle {
        Ok(h) => Ok(unsafe { File::from_raw_handle(h.0 as _) }),
        Err(e) => Err(anyhow!(
            "Failed to open {} pipe '{}': {}",
            if for_read { "input" } else { "output" },
            pipe_name,
            e
        )),
    }
}
