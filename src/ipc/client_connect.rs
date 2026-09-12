use super::*;

#[cfg(target_os = "windows")]
pub(crate) fn generate_one_time_ipc_token() -> ResultType<String> {
    use hbb_common::rand::{rngs::OsRng, RngCore as _};
    use std::fmt::Write as _;

    let mut random_bytes = [0u8; IPC_TOKEN_RANDOM_BYTES];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut random_bytes).map_err(|err| {
        hbb_common::anyhow::anyhow!(
            "failed to generate portable service ipc token from OsRng: {}",
            err
        )
    })?;

    let mut token = String::with_capacity(IPC_TOKEN_LEN);
    for byte in random_bytes {
        let _ = write!(token, "{:02x}", byte);
    }
    Ok(token)
}

#[cfg(target_os = "windows")]
pub(crate) fn constant_time_ipc_token_eq(expected: &str, candidate: &str) -> bool {
    if expected.len() != IPC_TOKEN_LEN || candidate.len() != IPC_TOKEN_LEN {
        return false;
    }
    expected
        .as_bytes()
        .iter()
        .zip(candidate.as_bytes().iter())
        .fold(0u8, |diff, (left, right)| diff | (*left ^ *right))
        == 0
}

#[cfg(target_os = "windows")]
pub(crate) async fn portable_service_ipc_handshake_as_client<T>(
    stream: &mut ConnectionTmpl<T>,
    token: &str,
) -> ResultType<()>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin,
{
    stream
        .send(&Data::DataPortableService(DataPortableService::AuthToken(
            token.to_owned(),
        )))
        .await?;
    match stream
        .next_timeout(PORTABLE_SERVICE_IPC_HANDSHAKE_TIMEOUT_MS)
        .await?
    {
        Some(Data::DataPortableService(DataPortableService::AuthResult(true))) => Ok(()),
        Some(Data::DataPortableService(DataPortableService::AuthResult(false))) => {
            bail!("portable service ipc handshake was rejected by server")
        }
        Some(_) | None => bail!("portable service ipc handshake returned an unexpected response"),
    }
}

#[cfg(target_os = "windows")]
pub(crate) async fn portable_service_ipc_handshake_as_server<T, F>(
    stream: &mut ConnectionTmpl<T>,
    mut validate_token: F,
) -> ResultType<()>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin,
    // Token validators must use `constant_time_ipc_token_eq` or an equivalent
    // fixed-length comparison; this handshake is part of the privilege boundary.
    F: FnMut(&str) -> bool,
{
    let authorized = match stream
        .next_timeout(PORTABLE_SERVICE_IPC_HANDSHAKE_TIMEOUT_MS)
        .await?
    {
        Some(Data::DataPortableService(DataPortableService::AuthToken(token))) => {
            validate_token(&token)
        }
        Some(_) | None => false,
    };
    stream
        .send(&Data::DataPortableService(DataPortableService::AuthResult(
            authorized,
        )))
        .await?;
    if !authorized {
        bail!("portable service ipc handshake failed")
    }
    Ok(())
}

#[inline]
pub(super) async fn connect_with_path(ms_timeout: u64, path: &str) -> ResultType<ConnectionTmpl<ConnClient>> {
    let client = timeout(ms_timeout, Endpoint::connect(path)).await??;
    Ok(ConnectionTmpl::new(client))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
pub(super) fn select_server_uid_for_user_main_ipc(
    server_uids: &[u32],
    active_uid: Option<u32>,
    prefer_root: bool,
) -> ResultType<u32> {
    let mut server_uids = server_uids.to_vec();
    server_uids.sort_unstable();
    server_uids.dedup();

    match server_uids.as_slice() {
        [] => {
            if let Some(uid) = active_uid {
                // If no `--server` processes are found but the active user is identifiable,
                // try the active user anyway because the main process may also listen on "" IPC.
                return Ok(uid);
            } else {
                bail!("No --server process found for user main IPC")
            }
        }
        [uid] => return Ok(*uid),
        _ => {}
    }

    if prefer_root && server_uids.contains(&0) {
        return Ok(0);
    }
    if let Some(active_uid) = active_uid.filter(|uid| server_uids.contains(uid)) {
        return Ok(active_uid);
    }
    bail!("Multiple --server processes found for user main IPC");
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn running_server_uids_for_current_exe() -> ResultType<Vec<u32>> {
    let current_exe = std::env::current_exe()?;
    let current_exe_path = std::fs::canonicalize(&current_exe)?;
    let current_pid = hbb_common::sysinfo::Pid::from_u32(std::process::id());
    let mut sys = hbb_common::sysinfo::System::new();
    sys.refresh_processes();
    let mut server_uids = Vec::new();
    for process in sys.processes().values() {
        if process.pid() == current_pid {
            continue;
        }
        if process.cmd().get(1).map_or(true, |arg| arg != "--server") {
            continue;
        }
        let Ok(process_path) = std::fs::canonicalize(process.exe()) else {
            continue;
        };
        if process_path != current_exe_path {
            continue;
        }
        let Some(uid) = process.user_id().map(|uid| **uid as u32) else {
            // Root CLI management commands need a stable matching `--server` target.
            // If this key process races during enumeration, failing the command is clearer
            // than silently skipping it; `--server` is not expected to exit frequently.
            bail!("Failed to read --server process uid");
        };
        server_uids.push(uid);
    }
    Ok(server_uids)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn user_main_ipc_server_uid() -> ResultType<u32> {
    let server_uids = running_server_uids_for_current_exe()?;
    #[cfg(target_os = "linux")]
    let prefer_root = crate::platform::linux::is_login_screen_wayland();
    #[cfg(target_os = "macos")]
    let prefer_root = false;
    select_server_uid_for_user_main_ipc(&server_uids, active_uid(), prefer_root)
}

pub async fn connect(ms_timeout: u64, postfix: &str) -> ResultType<ConnectionTmpl<ConnClient>> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let use_user_main_ipc = USE_USER_MAIN_IPC.with(|use_user_main| use_user_main.get());
        let is_root_main_ipc =
            unsafe { hbb_common::libc::geteuid() == 0 } && postfix.is_empty() && use_user_main_ipc;
        if is_root_main_ipc {
            let uid = user_main_ipc_server_uid()?;
            let path = Config::ipc_path_for_uid(uid, postfix);
            return connect_with_path(ms_timeout, &path).await;
        }
        let path = Config::ipc_path(postfix);
        return connect_with_path(ms_timeout, &path).await;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let path = Config::ipc_path(postfix);
        connect_with_path(ms_timeout, &path).await
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub async fn connect_for_uid(
    ms_timeout: u64,
    uid: u32,
    postfix: &str,
) -> ResultType<ConnectionTmpl<ConnClient>> {
    let path = Config::ipc_path_for_uid(uid, postfix);
    let conn = connect_with_path(ms_timeout, &path).await?;
    #[cfg(target_os = "macos")]
    if postfix.is_empty()
        && !authorize_user_server_process(conn.peer_uid(), conn.peer_pid(), uid)
    {
        bail!("Rejected user IPC peer for uid {}", uid);
    }
    Ok(conn)
}
