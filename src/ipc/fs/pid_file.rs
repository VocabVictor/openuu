use super::*;

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
pub(super) fn write_pid_file(path: &Path) -> ResultType<()> {
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
pub(super) fn read_pid_file_secure(path: &Path) -> Option<usize> {
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
pub(super) async fn probe_existing_listener(postfix: &str) -> bool {
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
