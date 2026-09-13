use crate::ipc::{Connection, ConnectionTmpl};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use hbb_common::{anyhow, bail, log, ResultType};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use hbb_common::{
    libc,
    tokio::io::{AsyncRead, AsyncWrite},
};
#[cfg(windows)]
use parity_tokio_ipc::SecurityAttributes;
#[cfg(windows)]
use std::io;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::io::RawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(windows)]
use windows::Win32::{Foundation::HANDLE, System::Pipes::GetNamedPipeClientProcessId};
#[cfg(windows)]
mod windows_conn;
#[cfg(windows)]
mod windows_auth;
#[cfg(windows)]
pub(crate) use windows_auth::*;
#[cfg(windows)]
pub(crate) use windows_conn::*;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_peer;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use unix_peer::*;
mod exe_path;
pub(crate) use exe_path::*;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
const UNAUTHORIZED_IPC_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
fn log_rejected_service_connection(postfix: &str, peer_uid: Option<u32>, active_uid: Option<u32>) {
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
    pub(super) fn peer_uid(&self) -> Option<u32> {
        peer_uid_from_fd(self.inner.get_ref().as_raw_fd())
    }

    fn service_authorization_status(&self) -> (bool, Option<u32>, Option<u32>) {
        let peer_uid = self.peer_uid();
        // On Linux, `_service` can use the cached active UID from the service loop for
        // stable config sync. Uinput does a fresh active-UID lookup in its own authorizer.
        let active_uid = active_uid();
        let authorized = peer_uid.is_some_and(|uid| is_allowed_service_peer_uid(uid, active_uid));
        (authorized, peer_uid, active_uid)
    }

    pub(super) fn peer_pid(&self) -> Option<u32> {
        peer_pid_from_fd(self.inner.get_ref().as_raw_fd())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn test_service_peer_uid_policy() {
        assert!(super::is_allowed_service_peer_uid(0, None));
        assert!(super::is_allowed_service_peer_uid(501, Some(501)));
        assert!(!super::is_allowed_service_peer_uid(502, Some(501)));
        assert!(!super::is_allowed_service_peer_uid(501, None));
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_server_peer_policy() {
        assert!(super::is_allowed_windows_session_scoped_peer(
            true, None, None
        ));
        assert!(super::is_allowed_windows_session_scoped_peer(
            false,
            Some(1),
            Some(1)
        ));
        assert!(!super::is_allowed_windows_session_scoped_peer(
            false,
            Some(1),
            Some(2)
        ));
        assert!(!super::is_allowed_windows_session_scoped_peer(
            false,
            None,
            Some(1)
        ));
    }

    #[test]
    #[cfg(windows)]
    fn test_windows_portable_service_peer_policy() {
        assert!(super::is_allowed_windows_portable_service_peer(
            Some(true),
            None,
            None
        ));
        assert!(!super::is_allowed_windows_portable_service_peer(
            Some(false),
            Some(1),
            Some(1)
        ));
        assert!(!super::is_allowed_windows_portable_service_peer(
            Some(false),
            Some(1),
            Some(2)
        ));
        assert!(!super::is_allowed_windows_portable_service_peer(
            None,
            Some(1),
            Some(1)
        ));
    }

    #[test]
    #[cfg(windows)]
    fn test_should_allow_everyone_create_on_windows_policy() {
        assert!(super::should_allow_everyone_create_on_windows(""));
        assert!(super::should_allow_everyone_create_on_windows("_service"));
        assert!(!super::should_allow_everyone_create_on_windows(
            "_portable_service"
        ));
    }

    #[test]
    #[cfg(windows)]
    fn test_executable_paths_match_windows_normalization() {
        let left = std::path::PathBuf::from(r"\\?\C:\Program Files\RustDesk\RustDesk.exe");
        let right = std::path::PathBuf::from(r"c:\program files\rustdesk\rustdesk.exe");
        assert!(super::executable_paths_match(&left, &right));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_os_str_eq_ignore_ascii_case_for_process_names() {
        assert!(super::os_str_eq_ignore_ascii_case(
            Some(std::ffi::OsStr::new("RustDesk")),
            Some(std::ffi::OsStr::new("rustdesk"))
        ));
        assert!(!super::os_str_eq_ignore_ascii_case(
            Some(std::ffi::OsStr::new("RustDesk")),
            Some(std::ffi::OsStr::new("service"))
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_console_owner_uid_matches_get_active_userid() {
        let console_uid =
            super::console_owner_uid().expect("/dev/console must have a resolvable uid");
        let raw_uid = crate::platform::macos::get_active_userid();
        let parsed_uid: u32 = raw_uid
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("failed to parse get_active_userid() output: '{raw_uid}'"));
        assert_eq!(parsed_uid, console_uid);
    }
}
