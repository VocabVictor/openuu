use super::*;

#[cfg(target_os = "macos")]
#[inline]
pub(super) fn macos_service_ipc_allows_gui_and_service_binaries(
    peer_exe: &Path,
    current_exe: &Path,
    postfix: &str,
) -> bool {
    if postfix != crate::POSTFIX_SERVICE {
        return false;
    }
    let Some(peer_dir) = peer_exe.parent() else {
        return false;
    };
    let Some(current_dir) = current_exe.parent() else {
        return false;
    };
    if !executable_paths_match(peer_dir, current_dir) {
        return false;
    }

    // On installed macOS builds, `_service` is listened by the `service` binary while the GUI
    // process connects from the app executable within the same app bundle.
    let gui_exe_name = std::ffi::OsString::from(crate::get_app_name());
    let gui_exe = gui_exe_name.as_os_str();
    let service_exe = std::ffi::OsStr::new("service");
    let allowed_exe = [Some(gui_exe), Some(service_exe)];
    let peer_name = peer_exe.file_name();
    let current_name = current_exe.file_name();
    allowed_exe
        .iter()
        .any(|name| os_str_eq_ignore_ascii_case(peer_name, *name))
        && allowed_exe
            .iter()
            .any(|name| os_str_eq_ignore_ascii_case(current_name, *name))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[inline]
pub(crate) fn is_allowed_service_peer_uid(peer_uid: u32, active_uid: Option<u32>) -> bool {
    // Root is allowed at the UID gate because the service side may run as root.
    // Callers still enforce executable matching before accepting service-scoped peers.
    peer_uid == 0 || active_uid.is_some_and(|uid| uid == peer_uid)
}

#[cfg(target_os = "macos")]
#[inline]
pub(super) fn console_owner_uid() -> Option<u32> {
    fs::metadata("/dev/console")
        .ok()
        .map(|metadata| metadata.uid())
}

#[cfg(target_os = "macos")]
#[inline]
pub(super) fn active_uid_strict() -> Option<u32> {
    // Prefer the filesystem metadata over parsing external command output.
    console_owner_uid()
}

#[cfg(target_os = "linux")]
#[inline]
pub(super) fn active_uid_strict() -> Option<u32> {
    let reported_uid_raw = crate::platform::linux::get_active_userid();
    let trimmed = reported_uid_raw.trim();
    if let Ok(uid) = trimmed.parse::<u32>() {
        return Some(uid);
    }
    if trimmed.is_empty() {
        log::debug!("Failed to resolve active user uid on linux: active uid is empty");
    } else {
        log::warn!("Failed to parse active user uid on linux: '{}'", trimmed);
    }
    None
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
pub(crate) fn active_uid() -> Option<u32> {
    active_uid_strict()
}

/// The active session uid read ONLY from the service-loop cache, never from a fresh (blocking) seat0
/// lookup. `None` on a cache miss. For hot, latency-sensitive, fail-closed re-auth on an async runtime
/// thread (the `_drm` per-frame re-auth), where a blocking `loginctl` per frame would stall the stream.
// Gated with the feature, not just the OS: the `_drm` per-frame re-auth is its only caller, so a
// drm-off Linux build would carry it as dead code and warn about it.
#[cfg(all(target_os = "linux", feature = "drm"))]
#[inline]
pub(crate) fn active_uid_cached() -> Option<u32> {
    crate::platform::linux::get_active_userid_cached()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
pub(crate) fn peer_uid_from_fd(fd: RawFd) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        return peer_cred_from_fd(fd).map(|cred| cred.uid as u32);
    }
    #[cfg(target_os = "macos")]
    {
        let mut uid = 0;
        let mut gid = 0;
        if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } == 0 {
            Some(uid as u32)
        } else {
            None
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[inline]
pub(super) fn peer_pid_from_fd(fd: RawFd) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        return peer_cred_from_fd(fd).and_then(|cred| (cred.pid > 0).then_some(cred.pid as u32));
    }
    #[cfg(target_os = "macos")]
    {
        let mut pid = 0;
        let mut len = std::mem::size_of::<libc::pid_t>() as _;
        let rc = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                &mut pid as *mut _ as *mut libc::c_void,
                &mut len,
            )
        };
        if rc == 0 && pid > 0 {
            Some(pid as _)
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
#[inline]
pub(super) fn peer_cred_from_fd(fd: RawFd) -> Option<libc::ucred> {
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as _;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        )
    };
    if rc == 0 {
        Some(cred)
    } else {
        None
    }
}
