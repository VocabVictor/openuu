use super::*;

#[inline]
pub(super) fn is_valid_portable_service_shmem_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= SHMEM_NAME_MAX_LEN
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

#[inline]
pub fn portable_service_shmem_arg(name: &str) -> String {
    format!("{SHMEM_ARG_PREFIX}{name}")
}

#[inline]
pub(super) fn is_valid_portable_service_ipc_token(token: &str) -> bool {
    token.len() == IPC_TOKEN_LEN
        && token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[inline]
pub(super) fn read_ipc_token_from_shmem(shmem: &SharedMemory) -> Option<String> {
    if shmem.len() < ADDR_IPC_TOKEN + IPC_TOKEN_LEN {
        log::error!(
            "Portable service shared memory too small: len={}, need>={}",
            shmem.len(),
            ADDR_IPC_TOKEN + IPC_TOKEN_LEN
        );
        return None;
    }
    unsafe {
        let ptr = shmem.as_ptr().add(ADDR_IPC_TOKEN);
        let bytes = slice::from_raw_parts(ptr, IPC_TOKEN_LEN);
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(IPC_TOKEN_LEN);
        if end == 0 {
            return None;
        }
        let token = std::str::from_utf8(&bytes[..end]).ok()?.to_owned();
        if is_valid_portable_service_ipc_token(&token) {
            Some(token)
        } else {
            None
        }
    }
}

#[inline]
pub(super) fn validate_runtime_shmem_layout(shmem: &SharedMemory) -> ResultType<()> {
    if shmem.len() < MIN_RUNTIME_SHMEM_LEN {
        bail!(
            "Portable service shared memory too small for runtime layout: len={}, need>={}",
            shmem.len(),
            MIN_RUNTIME_SHMEM_LEN
        );
    }
    Ok(())
}

#[inline]
pub(super) fn is_valid_capture_frame_length(shmem_len: usize, frame_len: usize) -> bool {
    let frame_capacity = shmem_len.saturating_sub(ADDR_CAPTURE_FRAME);
    frame_len > 0 && frame_len <= frame_capacity
}

#[inline]
pub(super) fn shared_memory_flink_path_by_name(name: &str) -> ResultType<PathBuf> {
    let mut dir = crate::platform::user_accessible_folder()?;
    dir = dir.join(hbb_common::config::APP_NAME.read().unwrap().clone());
    dir = dir.join(SHMEM_PARENT_DIR);
    Ok(dir.join(format!("shared_memory{}", name)))
}

#[inline]
pub(super) fn remove_shared_memory_flink_once(name: &str, log_on_error: bool, log_context: &str) -> bool {
    let flink = match shared_memory_flink_path_by_name(name) {
        Ok(path) => path,
        Err(err) => {
            if log_on_error {
                log::warn!(
                    "{} failed to resolve portable service shared-memory flink path for '{}': {}",
                    log_context,
                    name,
                    err
                );
            }
            return false;
        }
    };
    match std::fs::remove_file(&flink) {
        Ok(()) => {
            log::info!(
                "{} removed portable service shared-memory flink artifact: {:?}",
                log_context,
                flink
            );
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => {
            if log_on_error {
                log::warn!(
                    "{} failed to remove portable service shared-memory flink artifact {:?}: {}",
                    log_context,
                    flink,
                    err
                );
            }
            false
        }
    }
}

#[inline]
pub(super) fn write_ipc_token_to_shmem(shmem: &SharedMemory, token: &str) -> ResultType<()> {
    if !is_valid_portable_service_ipc_token(token) {
        bail!("Invalid portable service ipc token");
    }
    shmem.write(ADDR_IPC_TOKEN, token.as_bytes());
    Ok(())
}

#[inline]
pub(super) fn clear_ipc_token_in_shmem(shmem: &SharedMemory) {
    shmem.write(ADDR_IPC_TOKEN, &[0u8; IPC_TOKEN_LEN]);
}

#[inline]
pub(super) fn portable_service_arg_value_candidate_from_arg<'a>(
    arg: &'a str,
    prefix: &str,
) -> Option<&'a str> {
    let mut value = arg.strip_prefix(prefix)?;
    value = value.trim_start();
    value = value
        .strip_prefix('"')
        .or_else(|| value.strip_prefix('\''))
        .unwrap_or(value);
    value = value.split_whitespace().next().unwrap_or_default();
    value = value.trim_matches(|c| c == '"' || c == '\'');
    Some(value)
}

#[inline]
pub fn portable_service_shmem_name_from_args() -> Option<String> {
    for arg in std::env::args() {
        if let Some(value) = portable_service_arg_value_candidate_from_arg(&arg, SHMEM_ARG_PREFIX) {
            if is_valid_portable_service_shmem_name(value) {
                return Some(value.to_owned());
            }
            log::error!(
                "Invalid portable service shared memory name argument: '{}'",
                value
            );
            return None;
        }
    }
    None
}

#[inline]
pub fn has_portable_service_shmem_arg() -> bool {
    std::env::args().any(|arg| arg.starts_with(SHMEM_ARG_PREFIX))
}
