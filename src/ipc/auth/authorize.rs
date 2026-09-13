use super::*;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) const UNAUTHORIZED_IPC_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
pub(super) fn log_rejected_service_connection(postfix: &str, peer_uid: Option<u32>, active_uid: Option<u32>) {
    hbb_common::throttled_log!(
        UNAUTHORIZED_IPC_LOG_INTERVAL,
        warn,
        "Rejected unauthorized connection on protected service-scoped IPC channel: postfix={}, peer_uid={:?}, active_uid={:?}",
        postfix,
        peer_uid,
        active_uid
    );
}

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn log_rejected_uinput_connection(
    postfix: &str,
    peer_uid: Option<u32>,
    active_uid: Option<u32>,
) {
    hbb_common::throttled_log!(
        UNAUTHORIZED_IPC_LOG_INTERVAL,
        warn,
        "Rejected unauthorized connection on uinput ipc channel: postfix={}, peer_uid={:?}, active_uid={:?}",
        postfix,
        peer_uid,
        active_uid
    );
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn authorize_service_scoped_ipc_connection(stream: &Connection, postfix: &str) -> bool {
    let peer_pid = stream.peer_pid();
    let (authorized, peer_uid, active_uid) = stream.service_authorization_status();
    if !authorized {
        log_rejected_service_connection(postfix, peer_uid, active_uid);
        return false;
    }
    if let Err(err) = ensure_peer_executable_matches_current_by_pid_opt(peer_pid, postfix) {
        log::warn!(
            "Rejected unauthorized connection on protected service-scoped IPC channel due to executable mismatch: postfix={}, peer_pid={:?}, err={}",
            postfix,
            peer_pid,
            err
        );
        return false;
    }
    true
}

#[cfg(target_os = "macos")]
pub(crate) fn authorize_user_server_process(
    peer_uid: Option<u32>,
    peer_pid: Option<u32>,
    expected_uid: u32,
) -> bool {
    if peer_uid != Some(expected_uid) {
        return false;
    }
    let Some(peer_pid) = peer_pid else {
        return false;
    };
    let Ok(peer_exe) = peer_exe_canonical_path_by_pid(peer_pid) else {
        return false;
    };
    let expected_path = PathBuf::from(format!(
        "/Applications/{}.app/Contents/MacOS/{}",
        crate::get_app_name(),
        crate::get_app_name()
    ));
    let Ok(expected_path) = fs::canonicalize(expected_path) else {
        return false;
    };
    paths_refer_to_same_file(&peer_exe, &expected_path)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl<T> ConnectionTmpl<T>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin + std::os::unix::io::AsRawFd,
{
    pub(in crate::ipc) fn peer_uid(&self) -> Option<u32> {
        peer_uid_from_fd(self.inner.get_ref().as_raw_fd())
    }

    pub(super) fn service_authorization_status(&self) -> (bool, Option<u32>, Option<u32>) {
        let peer_uid = self.peer_uid();
        // On Linux, `_service` can use the cached active UID from the service loop for
        // stable config sync. Uinput does a fresh active-UID lookup in its own authorizer.
        let active_uid = active_uid();
        let authorized = peer_uid.is_some_and(|uid| is_allowed_service_peer_uid(uid, active_uid));
        (authorized, peer_uid, active_uid)
    }

    pub(in crate::ipc) fn peer_pid(&self) -> Option<u32> {
        peer_pid_from_fd(self.inner.get_ref().as_raw_fd())
    }
}
