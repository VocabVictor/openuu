#[cfg(target_os = "linux")]
use super::ipc_auth::active_uid;
use crate::ipc::{connect, Data};
use hbb_common::{config, log, ResultType};
use std::{
    ffi::CString,
    io::{Error, ErrorKind},
    os::unix::ffi::OsStrExt,
    path::Path,
};

mod parent_dir;
pub(crate) use parent_dir::*;
mod secure_dir;
pub(crate) use secure_dir::*;

struct FdGuard(i32);
impl Drop for FdGuard {
    fn drop(&mut self) {
        unsafe {
            hbb_common::libc::close(self.0);
        }
    }
}

#[inline]
pub(crate) fn get_pid_file(postfix: &str) -> String {
    let path = config::Config::ipc_path(postfix);
    format!("{}.pid", path)
}

// Purpose:
// - Write current process pid to pid file without following attacker-controlled symlinks.
// - Ensure the pid file is a regular file owned by the opened inode path.
//
// Approach:
// - Use libc open/fstat/write syscalls (FFI) so flags and inode validation are explicit.
// - Open file with O_NOFOLLOW/O_CLOEXEC and verify S_IFREG with fstat before write.
// - Keep unsafe scopes minimal and check syscall return values immediately.
//
// Main steps:
// 1) Secure-open pid file (without truncation).
// 2) Validate opened inode is a regular file owned by current euid.
// 3) Enforce pid file mode to 0600 and truncate via ftruncate after validation.
// 4) Write process id bytes through fd.
//
// Why not plain std::fs::write?
// - std::fs helpers cannot enforce this exact open-time hardening sequence
//   (especially "open with O_NOFOLLOW, then fstat the same opened inode").
//
// References:
// - open(2): O_NOFOLLOW/O_CLOEXEC/O_NONBLOCK
//   https://man7.org/linux/man-pages/man2/open.2.html
// - fstat(2): verify file type on opened fd
//   https://man7.org/linux/man-pages/man2/fstat.2.html
// - fchmod(2): enforce secure mode on reused pid file
//   https://man7.org/linux/man-pages/man2/fchmod.2.html
// - ftruncate(2): truncate after validation
//   https://man7.org/linux/man-pages/man2/ftruncate.2.html
// - write(2): write bytes via fd
//   https://man7.org/linux/man-pages/man2/write.2.html
fn write_pid_file(path: &Path) -> ResultType<()> {
    let path_c = CString::new(path.as_os_str().as_bytes().to_vec()).map_err(|err| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("invalid pid file path '{}': {}", path.display(), err),
        )
    })?;
    let flags = hbb_common::libc::O_WRONLY
        | hbb_common::libc::O_CREAT
        | hbb_common::libc::O_CLOEXEC
        | hbb_common::libc::O_NOFOLLOW
        | hbb_common::libc::O_NONBLOCK;
    let fd = unsafe { hbb_common::libc::open(path_c.as_ptr(), flags, 0o0600) };
    if fd < 0 {
        let os_err = std::io::Error::last_os_error();
        return Err(Error::new(
            os_err.kind(),
            format!(
                "failed to open pid file with no-follow '{}': {}",
                path.display(),
                os_err
            ),
        )
        .into());
    }
    let _fd_guard = FdGuard(fd);
    let mut stat: hbb_common::libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { hbb_common::libc::fstat(fd, &mut stat) } != 0 {
        let os_err = std::io::Error::last_os_error();
        return Err(Error::new(
            os_err.kind(),
            format!("failed to stat pid file '{}': {}", path.display(), os_err),
        )
        .into());
    }
    if (stat.st_mode & (hbb_common::libc::S_IFMT as hbb_common::libc::mode_t))
        != (hbb_common::libc::S_IFREG as hbb_common::libc::mode_t)
    {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            format!("pid file path is not a regular file: '{}'", path.display()),
        )
        .into());
    }
    let expected_uid = unsafe { hbb_common::libc::geteuid() as u32 };
    if stat.st_uid as u32 != expected_uid {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            format!(
                "pid file owner mismatch: expected uid {}, got {} for '{}'",
                expected_uid,
                stat.st_uid,
                path.display()
            ),
        )
        .into());
    }
    if unsafe { hbb_common::libc::fchmod(fd, 0o600) } != 0 {
        let os_err = std::io::Error::last_os_error();
        return Err(Error::new(
            os_err.kind(),
            format!("failed to chmod pid file '{}': {}", path.display(), os_err),
        )
        .into());
    }
    if unsafe { hbb_common::libc::ftruncate(fd, 0) } != 0 {
        let os_err = std::io::Error::last_os_error();
        return Err(Error::new(
            os_err.kind(),
            format!(
                "failed to truncate pid file '{}': {}",
                path.display(),
                os_err
            ),
        )
        .into());
    }

    let bytes = std::process::id().to_string();
    let buf = bytes.as_bytes();
    // `write(2)` is allowed to return a short write even for regular files.
    // PID content is tiny and usually written in one shot, but we still loop
    // until all bytes are persisted so this path is semantically correct.
    let mut written = 0usize;
    while written < buf.len() {
        let rc = unsafe {
            hbb_common::libc::write(
                fd,
                buf[written..].as_ptr() as *const hbb_common::libc::c_void,
                buf.len() - written,
            )
        };
        if rc < 0 {
            let os_err = std::io::Error::last_os_error();
            return Err(Error::new(
                os_err.kind(),
                format!("failed to write pid file '{}': {}", path.display(), os_err),
            )
            .into());
        }
        if rc == 0 {
            return Err(Error::new(
                ErrorKind::WriteZero,
                format!(
                    "failed to write pid file '{}': write returned 0 bytes",
                    path.display()
                ),
            )
            .into());
        }
        written += rc as usize;
    }
    Ok(())
}

#[inline]
pub(crate) fn write_pid(postfix: &str) {
    let path = std::path::PathBuf::from(get_pid_file(postfix));
    if let Err(err) = write_pid_file(&path) {
        log::warn!(
            "Failed to write pid file for postfix '{}', path='{}', err={}",
            postfix,
            path.display(),
            err
        );
    }
}

// Purpose:
// - Read pid file safely and avoid trusting symlink/non-regular files.
//
// Approach:
// - Use libc open/fstat/read syscalls (FFI) to control flags and inode checks.
// - Open path with O_NOFOLLOW, validate opened fd via fstat, then read and parse.
// - Keep unsafe scopes minimal and check syscall return values immediately.
//
// Main steps:
// 1) Secure-open pid file read-only.
// 2) Ensure fd points to regular file.
// 3) Read bytes and parse usize pid.
//
// References:
// - open(2): O_NOFOLLOW/O_CLOEXEC/O_NONBLOCK
//   https://man7.org/linux/man-pages/man2/open.2.html
// - fstat(2): validate S_IFREG on opened fd
//   https://man7.org/linux/man-pages/man2/fstat.2.html
// - read(2): read bytes via fd
//   https://man7.org/linux/man-pages/man2/read.2.html
#[inline]
fn read_pid_file_secure(path: &Path) -> Option<usize> {
    let path_c = CString::new(path.as_os_str().as_bytes().to_vec()).ok()?;
    let flags = hbb_common::libc::O_RDONLY
        | hbb_common::libc::O_CLOEXEC
        | hbb_common::libc::O_NOFOLLOW
        | hbb_common::libc::O_NONBLOCK;
    let fd = unsafe { hbb_common::libc::open(path_c.as_ptr(), flags) };
    if fd < 0 {
        return None;
    }
    let _fd_guard = FdGuard(fd);

    let mut stat: hbb_common::libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { hbb_common::libc::fstat(fd, &mut stat) } != 0 {
        return None;
    }
    if (stat.st_mode & (hbb_common::libc::S_IFMT as hbb_common::libc::mode_t))
        != (hbb_common::libc::S_IFREG as hbb_common::libc::mode_t)
    {
        return None;
    }

    let mut buffer = [0u8; 64];
    let read_len = unsafe {
        hbb_common::libc::read(
            fd,
            buffer.as_mut_ptr() as *mut hbb_common::libc::c_void,
            buffer.len(),
        )
    };
    if read_len <= 0 {
        return None;
    }
    let content = String::from_utf8_lossy(&buffer[..read_len as usize]).to_string();
    content.trim().parse::<usize>().ok()
}

#[inline]
async fn probe_existing_listener(postfix: &str) -> bool {
    let Ok(mut stream) = connect(1000, postfix).await else {
        return false;
    };
    if postfix != crate::POSTFIX_SERVICE {
        return true;
    }
    if stream.send(&Data::SyncConfig(None)).await.is_err() {
        return false;
    }
    matches!(
        stream.next_timeout(1000).await,
        Ok(Some(Data::SyncConfig(Some(_))))
    )
}

pub(crate) async fn check_pid(postfix: &str) -> bool {
    let pid_file = std::path::PathBuf::from(get_pid_file(postfix));
    if let Some(pid) = read_pid_file_secure(&pid_file) {
        if pid > 0 {
            let mut sys = hbb_common::sysinfo::System::new();
            sys.refresh_processes();
            if let Some(p) = sys.process(pid.into()) {
                if let Some(current) = sys.process((std::process::id() as usize).into()) {
                    if current.name() == p.name() && probe_existing_listener(postfix).await {
                        return true;
                    }
                }
            }
        }
    }
    if probe_existing_listener(postfix).await {
        return true;
    }
    // if not remove old ipc file, the new ipc creation will fail
    // if we remove a ipc file, but the old ipc process is still running,
    // new connection to the ipc will connect to new ipc, old connection to old ipc still keep alive
    if let Err(err) = remove_ipc_socket_via_secure_parent_fd(postfix) {
        log::debug!(
            "Failed to remove stale ipc socket via secure parent fd: postfix={}, err={}",
            postfix,
            err
        );
    }
    false
}

#[inline]
pub(crate) fn should_scrub_parent_entries_after_check_pid(
    should_scrub_parent_entries: bool,
    existing_listener_alive: bool,
) -> bool {
    should_scrub_parent_entries && !existing_listener_alive
}

#[cfg(test)]
mod tests {
    // Pins the HELPER's contract, which is all `new_drm_listener` consists of at that line -- not
    // the call site itself. Binding the real `/tmp/<app>-service/ipc_drm` from a test would collide
    // with a live root service, so "the listener still calls this" is not covered here.
    #[test]
    fn test_remove_ipc_entry_via_secure_parent_fd_clears_an_empty_directory_squatter() {
        let unique = format!(
            "rustdesk-ipc-entry-remove-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();
        let squatter = base.join("ipc_drm");
        std::fs::create_dir(&squatter).unwrap();

        // Positive control for the defect this closes: `remove_file` is `unlink(2)` and cannot
        // remove a directory. That is why the listener could not clear one, and then failed to
        // bind over it. Without this line a passing test would prove nothing.
        assert!(
            std::fs::remove_file(&squatter).is_err(),
            "remove_file must fail on a directory, or this test is vacuous"
        );
        assert!(squatter.is_dir());

        super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref()).unwrap();
        assert!(
            !squatter.exists(),
            "the fd-based removal picks AT_REMOVEDIR and clears it"
        );

        // Idempotent: this runs before every bind, so a path that is already gone is not an error.
        super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref()).unwrap();

        // The ORDINARY case, and the one the listener hits on every restart: a stale socket left by
        // the previous run, i.e. a regular file. Covered here because the other file-removal test
        // goes through `remove_parent_entry_via_fd` and the postfix path, not this entry point.
        std::fs::write(&squatter, b"stale").unwrap();
        super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref()).unwrap();
        assert!(!squatter.exists(), "a stale regular file is cleared too");

        // And the documented limit, pinned so the doc cannot drift: AT_REMOVEDIR is rmdir(2), so a
        // NON-empty squatter is reported, not cleared. The caller logs that and carries on; nothing
        // here should ever start deleting a tree it did not create.
        std::fs::create_dir(&squatter).unwrap();
        std::fs::write(squatter.join("planted"), b"x").unwrap();
        assert!(
            super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref())
                .is_err(),
            "a non-empty directory must be reported, not silently left as success"
        );
        assert!(squatter.join("planted").exists(), "and not deleted");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_write_pid_file_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let unique = format!(
            "rustdesk-ipc-pid-file-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();

        let target = base.join("target_pid");
        std::fs::write(&target, b"origin").unwrap();
        let link = base.join("pid_link");
        symlink(&target, &link).unwrap();

        let res = super::write_pid_file(&link);
        assert!(res.is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "origin");

        std::fs::remove_file(&link).ok();
        std::fs::remove_file(&target).ok();
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_ensure_secure_ipc_parent_dir_rejects_symlink_parent() {
        use std::os::unix::fs::symlink;

        let unique = format!(
            "rustdesk-ipc-secure-dir-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        let real_dir = base.join("real");
        let link_dir = base.join("link");
        std::fs::create_dir_all(&real_dir).unwrap();
        symlink(&real_dir, &link_dir).unwrap();
        let ipc_path = link_dir.join("ipc_service");
        let res =
            super::ensure_secure_ipc_parent_dir(ipc_path.to_string_lossy().as_ref(), "_service");
        assert!(res.is_err());
        std::fs::remove_file(&link_dir).ok();
        std::fs::remove_dir_all(&real_dir).ok();
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_ensure_secure_ipc_parent_dir_creates_parent_with_expected_mode() {
        use std::os::unix::fs::PermissionsExt;

        let unique = format!(
            "rustdesk-ipc-secure-dir-create-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();

        // Intentionally choose a parent that does not exist to exercise the ENOENT -> mkdir branch.
        let parent_dir = base.join("parent");
        assert!(!parent_dir.exists());
        let ipc_path = parent_dir.join("ipc");

        let res = super::ensure_secure_ipc_parent_dir(ipc_path.to_string_lossy().as_ref(), "");
        // Restrictive umask can make mkdir create a stricter initial mode. In that case
        // ensure_secure_ipc_parent_dir repairs it with fchmod and may request a scrub.
        res.unwrap();

        let md = std::fs::metadata(&parent_dir).unwrap();
        assert!(md.is_dir());
        let mode = md.permissions().mode() & 0o777;
        assert_eq!(mode, 0o0700);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_scrub_preexisting_ipc_parent_entries_only_removes_target_postfix_artifacts() {
        use std::os::unix::ffi::OsStrExt;

        let unique = format!(
            "rustdesk-ipc-scrub-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();

        let ipc_file = base.join("ipc_service");
        let ipc_pid_file = base.join("ipc_service.pid");
        let ipc_other_postfix_file = base.join("ipc_uinput_1");
        let keep_file = base.join("keep.txt");
        let keep_dir = base.join("keep_dir");

        std::fs::write(&ipc_file, b"socket-placeholder").unwrap();
        std::fs::write(&ipc_pid_file, b"1234").unwrap();
        std::fs::write(&ipc_other_postfix_file, b"other-postfix").unwrap();
        std::fs::write(&keep_file, b"keep").unwrap();
        std::fs::create_dir_all(&keep_dir).unwrap();

        let base_c = std::ffi::CString::new(base.as_os_str().as_bytes().to_vec()).unwrap();
        let base_fd = super::open_ipc_parent_dir_fd(&base_c).unwrap();
        let _base_guard = super::FdGuard(base_fd);
        super::scrub_preexisting_ipc_parent_entries(base_fd, &base, "_service").unwrap();

        assert!(!ipc_file.exists());
        assert!(!ipc_pid_file.exists());
        assert!(ipc_other_postfix_file.exists());
        assert!(keep_file.exists());
        assert!(keep_dir.exists());

        std::fs::remove_file(&ipc_other_postfix_file).ok();
        std::fs::remove_file(&keep_file).ok();
        std::fs::remove_dir_all(&keep_dir).ok();
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_scrub_preexisting_ipc_parent_entries_should_bind_to_opened_inode_not_path() {
        use std::os::unix::ffi::OsStrExt;

        let unique = format!(
            "rustdesk-ipc-scrub-fd-bind-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();

        let trusted_parent = base.join("trusted_parent");
        let trusted_parent_moved = base.join("trusted_parent_moved");
        let attacker_parent = base.join("attacker_parent");
        std::fs::create_dir_all(&trusted_parent).unwrap();
        std::fs::create_dir_all(&attacker_parent).unwrap();

        let trusted_ipc_file = trusted_parent.join("ipc_service");
        let attacker_ipc_file = attacker_parent.join("ipc_service");
        std::fs::write(&trusted_ipc_file, b"trusted").unwrap();
        std::fs::write(&attacker_ipc_file, b"attacker").unwrap();

        let trusted_parent_c =
            std::ffi::CString::new(trusted_parent.as_os_str().as_bytes().to_vec()).unwrap();
        let trusted_parent_fd = super::open_ipc_parent_dir_fd(&trusted_parent_c).unwrap();
        let _trusted_parent_guard = super::FdGuard(trusted_parent_fd);

        // Swap the path after the trusted inode has been opened.
        std::fs::rename(&trusted_parent, &trusted_parent_moved).unwrap();
        std::fs::rename(&attacker_parent, &trusted_parent).unwrap();

        super::scrub_preexisting_ipc_parent_entries(trusted_parent_fd, &trusted_parent, "_service")
            .unwrap();

        // Expected secure behavior: scrub should target the inode that was opened before path swap.
        assert!(
            !trusted_parent_moved.join("ipc_service").exists(),
            "trusted inode artifact should be removed even after path swap"
        );
        assert!(
            trusted_parent.join("ipc_service").exists(),
            "path-swapped attacker directory should not be scrubbed"
        );

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_ensure_secure_ipc_parent_dir_keeps_service_artifacts_before_liveness_probe() {
        use std::os::unix::fs::PermissionsExt;

        let unique = format!(
            "rustdesk-ipc-secure-dir-order-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();

        let parent_dir = base.join("service_parent");
        std::fs::create_dir_all(&parent_dir).unwrap();
        // Trigger "had_untrusted_service_parent_mode".
        std::fs::set_permissions(&parent_dir, std::fs::Permissions::from_mode(0o777)).unwrap();

        let ipc_file = parent_dir.join("ipc_service");
        let ipc_pid_file = parent_dir.join("ipc_service.pid");
        std::fs::write(&ipc_file, b"socket-placeholder").unwrap();
        std::fs::write(&ipc_pid_file, b"1234").unwrap();

        let res =
            super::ensure_secure_ipc_parent_dir(ipc_file.to_string_lossy().as_ref(), "_service");
        assert_eq!(res.unwrap(), true);

        // Parent hardening should run first; artifacts should stay until liveness probe completes.
        assert!(ipc_file.exists(), "ipc socket marker should be preserved");
        assert!(ipc_pid_file.exists(), "pid marker should be preserved");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_ensure_secure_ipc_parent_dir_marks_non_service_mode_repair_for_scrub() {
        use std::os::unix::fs::PermissionsExt;

        let unique = format!(
            "rustdesk-ipc-nonservice-mode-repair-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let base = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&base).unwrap();

        let parent_dir = base.join("non_service_parent");
        std::fs::create_dir_all(&parent_dir).unwrap();
        std::fs::set_permissions(&parent_dir, std::fs::Permissions::from_mode(0o755)).unwrap();

        let ipc_file = parent_dir.join("ipc");
        std::fs::write(&ipc_file, b"socket-placeholder").unwrap();

        let res = super::ensure_secure_ipc_parent_dir(ipc_file.to_string_lossy().as_ref(), "");
        assert_eq!(res.unwrap(), true);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn test_should_scrub_parent_entries_after_check_pid_only_when_requested_and_not_alive() {
        assert!(!super::should_scrub_parent_entries_after_check_pid(
            false, false
        ));
        assert!(!super::should_scrub_parent_entries_after_check_pid(
            false, true
        ));
        assert!(super::should_scrub_parent_entries_after_check_pid(
            true, false
        ));
        assert!(!super::should_scrub_parent_entries_after_check_pid(
            true, true
        ));
    }
}
