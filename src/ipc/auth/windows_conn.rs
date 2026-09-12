#[cfg(windows)]
use super::*;

#[cfg(windows)]
#[inline]
pub(crate) fn should_allow_everyone_create_on_windows(postfix: &str) -> bool {
    postfix.is_empty() || hbb_common::config::is_service_ipc_postfix(postfix)
}

#[cfg(windows)]
#[inline]
pub(crate) fn portable_service_listener_security_attributes() -> io::Result<SecurityAttributes> {
    let user_sid = crate::platform::windows::current_process_user_sid_string().map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("failed to resolve current process SID: {}", err),
        )
    })?;
    debug_assert!(
        user_sid.starts_with("S-1-")
            && user_sid
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-'),
        "current_process_user_sid_string returned a non-SDDL SID: {}",
        user_sid
    );
    // SDDL:
    // - `D:P`                => protected DACL (no inherited ACEs)
    // - `(A;;GA;;;SY)`       => allow GENERIC_ALL to LocalSystem
    // - `(A;;GA;;;{user_sid})` => allow GENERIC_ALL to current process user SID
    // References:
    // - Security Descriptor String Format: https://learn.microsoft.com/en-us/windows/win32/secauthz/security-descriptor-string-format
    // - ACE strings in SDDL: https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-strings
    let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{user_sid})");
    SecurityAttributes::from_sddl(&sddl).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "failed to build portable service listener security attributes from SDDL '{}': {}",
                sddl, err
            ),
        )
    })
}

#[cfg(windows)]
impl ConnectionTmpl<parity_tokio_ipc::Connection> {
    fn peer_pid(&self) -> Option<u32> {
        let pipe_handle = self.inner.get_ref().as_raw_handle();
        if pipe_handle.is_null() {
            return None;
        }
        let mut pid = 0u32;
        let ok = unsafe { GetNamedPipeClientProcessId(HANDLE(pipe_handle), &mut pid as *mut u32) }
            .is_ok();
        if ok && pid != 0 {
            Some(pid)
        } else {
            None
        }
    }

    pub(super) fn server_authorization_status(
        &self,
    ) -> (
        bool,
        Option<u32>,
        Option<u32>,
        Option<u32>,
        Option<bool>,
        Option<bool>,
    ) {
        let peer_pid = self.peer_pid();
        let server_session_id = crate::platform::windows::get_current_process_session_id();
        let peer_session_id =
            peer_pid.and_then(crate::platform::windows::get_session_id_of_process);
        let peer_is_system_result =
            peer_pid.map(crate::platform::windows::is_process_running_as_system);
        let peer_is_system = peer_is_system_result
            .as_ref()
            .and_then(|r| r.as_ref().ok().copied());
        let session_authorized = is_allowed_windows_session_scoped_peer(
            peer_is_system.unwrap_or(false),
            peer_session_id,
            server_session_id,
        );
        let peer_is_elevated_result = if session_authorized {
            None
        } else {
            peer_pid.map(|pid| crate::platform::windows::is_elevated(Some(pid)))
        };
        let peer_is_elevated = peer_is_elevated_result
            .as_ref()
            .and_then(|r| r.as_ref().ok().copied());
        if server_session_id.is_none()
            && !peer_is_system.unwrap_or(false)
            && !peer_is_elevated.unwrap_or(false)
        {
            // When the server session id cannot be determined, the session-id allow-path is
            // disabled and only privileged peers can be authorized.
            log::debug!(
                "IPC authorization: server session id unavailable; rejecting non-privileged peer, peer_pid={:?}, peer_session_id={:?}",
                peer_pid,
                peer_session_id
            );
        }
        // Main IPC trusts same-session peers, LocalSystem, and elevated administrators.
        // Service-scoped IPC channels keep their own stricter authorization paths.
        let authorized = session_authorized || peer_is_elevated.unwrap_or(false);
        if !authorized {
            if let (Some(pid), Some(Err(err))) = (peer_pid, peer_is_system_result.as_ref()) {
                log::debug!(
                    "Failed to determine whether peer process is SYSTEM, pid={}, err={}",
                    pid,
                    err
                );
            }
            if let (Some(pid), Some(Err(err))) = (peer_pid, peer_is_elevated_result.as_ref()) {
                log::debug!(
                    "Failed to determine whether peer process is elevated, pid={}, err={}",
                    pid,
                    err
                );
            }
        }
        (
            authorized,
            peer_pid,
            peer_session_id,
            server_session_id,
            peer_is_system,
            peer_is_elevated,
        )
    }

    pub(crate) fn service_authorization_status_for_session(
        &self,
        expected_active_session_id: Option<u32>,
    ) -> (bool, Option<u32>, Option<u32>, Option<bool>) {
        let peer_pid = self.peer_pid();
        let peer_session_id =
            peer_pid.and_then(crate::platform::windows::get_session_id_of_process);
        let peer_is_system_result =
            peer_pid.map(crate::platform::windows::is_process_running_as_system);
        let peer_is_system = peer_is_system_result
            .as_ref()
            .and_then(|r| r.as_ref().ok().copied());
        let authorized = is_allowed_windows_session_scoped_peer(
            peer_is_system.unwrap_or(false),
            peer_session_id,
            expected_active_session_id,
        );
        if !authorized {
            if let (Some(pid), Some(Err(err))) = (peer_pid, peer_is_system_result.as_ref()) {
                log::debug!(
                    "Failed to determine whether peer process is SYSTEM, pid={}, err={}",
                    pid,
                    err
                );
            }
        }
        (authorized, peer_pid, peer_session_id, peer_is_system)
    }

    pub(crate) fn portable_service_authorization_status_for_session(
        &self,
        expected_active_session_id: Option<u32>,
    ) -> (bool, Option<u32>, Option<u32>, Option<bool>) {
        // Portable-service policy:
        // only SYSTEM peers are allowed.
        let (_service_authorized, peer_pid, peer_session_id, peer_is_system) =
            self.service_authorization_status_for_session(expected_active_session_id);
        (
            is_allowed_windows_portable_service_peer(
                peer_is_system,
                peer_session_id,
                expected_active_session_id,
            ),
            peer_pid,
            peer_session_id,
            peer_is_system,
        )
    }
}
