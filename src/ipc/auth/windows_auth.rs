#[cfg(windows)]
use super::*;

#[cfg(target_os = "windows")]
#[inline]
pub(super) fn windows_portable_service_ipc_allows_logon_helper_executable(
    _peer_exe: &Path,
    postfix: &str,
) -> bool {
    if postfix != "_portable_service" {
        return false;
    }
    #[cfg(feature = "flutter")]
    {
        false
    }
}

#[cfg(windows)]
#[inline]
pub(crate) fn is_allowed_windows_session_scoped_peer(
    client_is_system: bool,
    client_session_id: Option<u32>,
    expected_session_id: Option<u32>,
) -> bool {
    client_is_system
        || matches!(
            (client_session_id, expected_session_id),
            (Some(client), Some(expected)) if client == expected
        )
}

#[cfg(windows)]
#[inline]
pub(super) fn is_allowed_windows_portable_service_peer(
    client_is_system: Option<bool>,
    _client_session_id: Option<u32>,
    _expected_session_id: Option<u32>,
) -> bool {
    // Portable-service listener DACL includes SYSTEM and current-process SID.
    // In the portable-service path, current process is expected to run as SYSTEM,
    // and the higher-layer peer policy stays SYSTEM-only.
    matches!(client_is_system, Some(true))
}

#[cfg(windows)]
#[inline]
pub(crate) fn log_rejected_windows_ipc_connection(
    postfix: &str,
    peer_pid: Option<u32>,
    peer_session_id: Option<u32>,
    expected_session_id: Option<u32>,
    peer_is_system: Option<bool>,
    peer_is_elevated: Option<bool>,
) {
    hbb_common::throttled_log!(
        UNAUTHORIZED_IPC_LOG_INTERVAL,
        warn,
        "Rejected unauthorized connection on ipc channel: postfix={}, peer_pid={:?}, peer_session_id={:?}, expected_session_id={:?}, peer_is_system={:?}, peer_is_elevated={:?}",
        postfix,
        peer_pid,
        peer_session_id,
        expected_session_id,
        peer_is_system,
        peer_is_elevated
    );
}

#[cfg(windows)]
pub(crate) fn authorize_windows_main_ipc_connection(stream: &Connection, postfix: &str) -> bool {
    let (
        authorized,
        peer_pid,
        peer_session_id,
        server_session_id,
        peer_is_system,
        peer_is_elevated,
    ) = stream.server_authorization_status();
    if !authorized {
        log_rejected_windows_ipc_connection(
            postfix,
            peer_pid,
            peer_session_id,
            server_session_id,
            peer_is_system,
            peer_is_elevated,
        );
        return false;
    }
    if let Err(err) = ensure_peer_executable_matches_current_by_pid_opt(peer_pid, postfix) {
        log::warn!(
            "Rejected unauthorized connection on ipc channel due to executable mismatch: postfix={}, peer_pid={:?}, err={}",
            postfix,
            peer_pid,
            err
        );
        return false;
    }
    true
}

#[cfg(windows)]
pub(crate) fn authorize_windows_portable_service_ipc_connection(
    stream: &Connection,
    postfix: &str,
) -> bool {
    // Portable service IPC policy:
    // - only SYSTEM peers are authorized by is_allowed_windows_portable_service_peer()
    // - expected_session_id is still collected for diagnostics and identity checks
    // - final privilege boundary is enforced by named-pipe ACL + one-time token handshake
    // - when peer identity is unavailable on some hosts, executable verification remains
    //   best-effort telemetry (not fail-closed) to avoid breaking valid SYSTEM bootstrap
    //   flows that cannot be fully introspected
    let expected_session_id = crate::platform::windows::get_current_process_session_id();
    let (authorized, peer_pid, peer_session_id, peer_is_system) =
        stream.portable_service_authorization_status_for_session(expected_session_id);
    if !authorized {
        // Session lookup may succeed while SYSTEM identity lookup fails, so only the
        // SYSTEM identity result determines whether peer identity is unavailable here.
        // Don't use `peer_pid.is_some() && peer_session_id.is_none() && peer_is_system.is_none();` here.
        let identity_unavailable = peer_pid.is_some() && peer_is_system.is_none();
        if identity_unavailable {
            // In portable-service startup, resolving SYSTEM peer identity may fail on some hosts.
            // `ProcessIdToSessionId` can still succeed while `OpenProcessToken(TOKEN_QUERY)` is
            // denied by the peer token DACL or missing privileges. Treat that partial identity
            // failure as unavailable and defer final authorization to pipe ACL + token handshake.
            if let Err(err) = ensure_peer_executable_matches_current_by_pid_opt(peer_pid, postfix) {
                log::warn!(
                    "Portable service ipc peer identity unavailable and executable verification failed; continue with ACL+token-gated flow: postfix={}, peer_pid={:?}, err={}",
                    postfix,
                    peer_pid,
                    err
                );
            } else {
                log::warn!(
                    "Portable service ipc peer identity unavailable; executable verification matched, continue with ACL+token-gated flow: postfix={}, peer_pid={:?}, expected_session_id={:?}",
                    postfix,
                    peer_pid,
                    expected_session_id
                );
            }
            return true;
        }
        log::warn!(
            "Rejected unauthorized connection on portable service ipc channel: postfix={}, peer_pid={:?}, peer_session_id={:?}, expected_session_id={:?}, peer_is_system={:?}",
            postfix,
            peer_pid,
            peer_session_id,
            expected_session_id,
            peer_is_system
        );
        return false;
    }
    true
}
