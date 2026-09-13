use super::*;

#[cfg(target_os = "linux")]
#[inline]
pub(crate) fn terminal_count_candidate_uids(effective_uid: u32) -> Vec<u32> {
    if effective_uid != 0 {
        return vec![effective_uid];
    }
    let mut candidates = Vec::with_capacity(2);
    if let Some(uid) = active_uid().filter(|uid| *uid != 0) {
        candidates.push(uid);
    }
    candidates.push(0);
    candidates
}

#[inline]
pub(super) fn expected_ipc_parent_mode(postfix: &str) -> u32 {
    if config::is_service_ipc_postfix(postfix) {
        0o0711
    } else {
        0o0700
    }
}

pub(super) fn open_ipc_parent_dir_fd(parent_c: &CString) -> std::io::Result<i32> {
    let fd = unsafe {
        hbb_common::libc::open(
            parent_c.as_ptr(),
            hbb_common::libc::O_RDONLY
                | hbb_common::libc::O_DIRECTORY
                | hbb_common::libc::O_CLOEXEC
                | hbb_common::libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

// Remove one preexisting IPC artifact via an already-opened parent directory FD.
//
// Security intent:
// - Bind cleanup to the exact parent inode that passed O_NOFOLLOW + fstat checks.
// - Avoid path-based TOCTOU during scrub (e.g., parent path rename/swap race).
//
// Flow:
// 1) fstatat(..., AT_SYMLINK_NOFOLLOW) to inspect the target entry under parent_fd.
// 2) Decide file vs directory from st_mode.
// 3) unlinkat relative to parent_fd (AT_REMOVEDIR for directories).
//
// Error policy:
// - NotFound is treated as benign (already removed / raced away).
// - Other errors are surfaced explicitly.
pub(super) fn remove_parent_entry_via_fd(
    parent_fd: i32,
    parent_dir: &Path,
    entry_name: &str,
) -> ResultType<()> {
    if entry_name.contains('/') {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "invalid ipc parent entry name (contains '/'): parent={}, entry={}",
                parent_dir.display(),
                entry_name
            ),
        )
        .into());
    }
    let entry_c = CString::new(entry_name.as_bytes().to_vec()).map_err(|err| {
        Error::new(
            ErrorKind::InvalidInput,
            format!(
                "invalid ipc parent entry name: parent={}, entry={}, err={}",
                parent_dir.display(),
                entry_name,
                err
            ),
        )
    })?;
    let mut stat: hbb_common::libc::stat = unsafe { std::mem::zeroed() };
    let stat_rc = unsafe {
        hbb_common::libc::fstatat(
            parent_fd,
            entry_c.as_ptr(),
            &mut stat,
            hbb_common::libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if stat_rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == ErrorKind::NotFound {
            return Ok(());
        }
        return Err(Error::new(
            err.kind(),
            format!(
                "failed to stat preexisting ipc parent dir entry by fd: parent={}, entry={}, err={}",
                parent_dir.display(),
                entry_name,
                err
            ),
        )
        .into());
    }

    let is_dir = (stat.st_mode & (hbb_common::libc::S_IFMT as hbb_common::libc::mode_t))
        == hbb_common::libc::S_IFDIR;
    let unlink_flags = if is_dir {
        hbb_common::libc::AT_REMOVEDIR
    } else {
        0
    };
    let unlink_rc =
        unsafe { hbb_common::libc::unlinkat(parent_fd, entry_c.as_ptr(), unlink_flags) };
    if unlink_rc != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == ErrorKind::NotFound {
            return Ok(());
        }
        return Err(Error::new(
            err.kind(),
            format!(
                "failed to remove preexisting ipc parent dir entry by fd: parent={}, entry={}, err={}",
                parent_dir.display(),
                entry_name,
                err
            ),
        )
        .into());
    }
    Ok(())
}

pub(super) fn scrub_preexisting_ipc_parent_entries(
    parent_fd: i32,
    parent_dir: &Path,
    postfix: &str,
) -> ResultType<()> {
    let ipc_basename = format!("ipc{}", postfix);
    remove_parent_entry_via_fd(parent_fd, parent_dir, &ipc_basename)?;
    remove_parent_entry_via_fd(parent_fd, parent_dir, &format!("{}.pid", ipc_basename))?;
    Ok(())
}

/// Remove one entry from the IPC parent directory through a no-follow fd on that directory.
///
/// Prefer this over `std::fs::remove_file` for anything about to be bound: `remove_file` is
/// `unlink(2)`, which returns EISDIR against a directory-typed squatter and leaves it in place,
/// and the bind that follows then fails EADDRINUSE. `remove_parent_entry_via_fd` fstats the
/// entry first and picks `AT_REMOVEDIR` when it needs to.
///
/// `AT_REMOVEDIR` is `rmdir(2)`, so the directory case this closes is the EMPTY one; a non-empty
/// squatter still yields ENOTEMPTY and still blocks the bind that follows. That is deliberate, and
/// the "obvious" fix is worse than the bug: removing it recursively would be root deleting a tree
/// an unprivileged process planted. What the caller gains there is a named error to log ahead of
/// the bind's own failure, not a successful bind.
pub(crate) fn remove_ipc_entry_via_secure_parent_fd(path: &str) -> ResultType<()> {
    let entry_name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("invalid ipc path: {path}")))?
        .to_owned();
    let parent_dir = Path::new(path)
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("invalid ipc path: {path}")))?;
    let parent_c = CString::new(parent_dir.as_os_str().as_bytes().to_vec())?;
    let fd = match open_ipc_parent_dir_fd(&parent_c) {
        Ok(fd) => fd,
        Err(open_err) => {
            if open_err.kind() == ErrorKind::NotFound {
                return Ok(());
            }
            return Err(Error::new(
                open_err.kind(),
                format!(
                    "failed to open ipc parent dir for stale socket cleanup (no-follow): path={}, parent={}, err={}",
                    path,
                    parent_dir.display(),
                    open_err
                ),
            )
            .into());
        }
    };
    let _fd_guard = FdGuard(fd);
    remove_parent_entry_via_fd(fd, parent_dir, &entry_name)
}

pub(super) fn remove_ipc_socket_via_secure_parent_fd(postfix: &str) -> ResultType<()> {
    remove_ipc_entry_via_secure_parent_fd(&config::Config::ipc_path(postfix))
}
