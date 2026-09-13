use super::*;

// Purpose:
// - Harden the IPC parent directory before creating/listening socket files.
// - Prevent symlink/path-race abuse and reject unsafe owner/mode.
//
// Approach:
// - Open parent dir with O_NOFOLLOW/O_DIRECTORY and operate on that fd.
// - Validate inode type/owner/mode via fstat.
// - For protected service postfix, optionally adopt owner (root only), then scrub stale
//   rustdesk IPC artifacts when directory trust boundary changed.
//
// Main steps:
// 1) Resolve parent path and open/create directory securely.
// 2) Verify directory inode type and owner uid.
// 3) Enforce expected mode via fchmod on opened fd.
// 4) Scrub stale IPC artifacts when owner/mode was unsafe before hardening.
//
// References:
// - open(2): O_NOFOLLOW/O_DIRECTORY/O_CLOEXEC
//   https://man7.org/linux/man-pages/man2/open.2.html
// - fstat(2): verify file type/metadata on opened fd
//   https://man7.org/linux/man-pages/man2/fstat.2.html
// - fchown(2): adopt ownership when running as root
//   https://man7.org/linux/man-pages/man2/chown.2.html
// - fchmod(2): enforce exact mode on opened fd
//   https://man7.org/linux/man-pages/man2/fchmod.2.html
pub(crate) fn ensure_secure_ipc_parent_dir(path: &str, postfix: &str) -> ResultType<bool> {
    let parent_dir = Path::new(path)
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("invalid ipc path: {path}")))?;
    // Harden against common TOCTOU by opening the parent directory with O_NOFOLLOW (so the parent
    // itself cannot be a symlink) and then operating on its FD (fstat/fchown/fchmod). This ensures
    // we mutate the inode we opened, though it does not protect against symlinks in ancestor path
    // components.
    let parent_c = CString::new(parent_dir.as_os_str().as_bytes().to_vec())?;
    let fd = match open_ipc_parent_dir_fd(&parent_c) {
        Ok(fd) => fd,
        Err(open_err) => {
            // If the directory doesn't exist yet, create it with the expected mode. The parent
            // dir is intended to be a single-level /tmp path, so mkdir is sufficient here.
            if open_err.raw_os_error() == Some(hbb_common::libc::ENOENT) {
                let expected_mode = expected_ipc_parent_mode(postfix);
                let rc = unsafe {
                    hbb_common::libc::mkdir(
                        parent_c.as_ptr(),
                        expected_mode as hbb_common::libc::mode_t,
                    )
                };
                if rc != 0 {
                    let mkdir_err = std::io::Error::last_os_error();
                    // Handle a race where another process created the directory first.
                    if mkdir_err.raw_os_error() != Some(hbb_common::libc::EEXIST) {
                        return Err(Error::new(
                            mkdir_err.kind(),
                            format!(
                                "failed to mkdir ipc parent dir: postfix={}, parent={}, err={}",
                                postfix,
                                parent_dir.display(),
                                mkdir_err
                            ),
                        )
                        .into());
                    }
                }
                match open_ipc_parent_dir_fd(&parent_c) {
                    Ok(fd) => fd,
                    Err(err) => {
                        return Err(Error::new(
                            err.kind(),
                            format!(
                                "failed to open ipc parent dir (no-follow): postfix={}, parent={}, err={}",
                                postfix,
                                parent_dir.display(),
                                err
                            ),
                        )
                        .into());
                    }
                }
            } else {
                return Err(Error::new(
                    open_err.kind(),
                    format!(
                        "failed to open ipc parent dir (no-follow): postfix={}, parent={}, err={}",
                        postfix,
                        parent_dir.display(),
                        open_err
                    ),
                )
                .into());
            }
        }
    };
    let _fd_guard = FdGuard(fd);

    let mut st: hbb_common::libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { hbb_common::libc::fstat(fd, &mut st as *mut _) } != 0 {
        let os_err = std::io::Error::last_os_error();
        return Err(Error::new(
            os_err.kind(),
            format!(
                "failed to stat ipc parent dir: postfix={}, parent={}, err={}",
                postfix,
                parent_dir.display(),
                os_err
            ),
        )
        .into());
    }
    let mode = st.st_mode as u32;
    let is_dir = (mode & (hbb_common::libc::S_IFMT as u32)) == (hbb_common::libc::S_IFDIR as u32);
    if !is_dir {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            format!(
                "ipc parent is not directory: postfix={}, parent={}",
                postfix,
                parent_dir.display()
            ),
        )
        .into());
    }

    let expected_uid = unsafe { hbb_common::libc::geteuid() as u32 };
    let mut owner_uid = st.st_uid as u32;
    let mut adopted_foreign_service_parent = false;
    // Service-scoped IPC may be created by different privilege contexts historically.
    // If running as root on protected service postfix, try adopting ownership first.
    if owner_uid != expected_uid && expected_uid == 0 && config::is_service_ipc_postfix(postfix) {
        let rc = unsafe {
            hbb_common::libc::fchown(
                fd,
                expected_uid as hbb_common::libc::uid_t,
                hbb_common::libc::gid_t::MAX,
            )
        };
        if rc == 0 {
            let mut st2: hbb_common::libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { hbb_common::libc::fstat(fd, &mut st2 as *mut _) } == 0 {
                owner_uid = st2.st_uid as u32;
                st = st2;
                adopted_foreign_service_parent = true;
            }
        } else {
            // Keep behavior unchanged; capture errno to ease diagnosing why chown failed.
            let err = std::io::Error::last_os_error();
            log::warn!(
                "Failed to chown ipc parent dir, parent={}, postfix={}, expected_uid={}, rc={}, err={:?}",
                parent_dir.display(),
                postfix,
                expected_uid,
                rc,
                err
            );
        }
    }
    if owner_uid != expected_uid {
        return Err(Error::new(
            ErrorKind::PermissionDenied,
            format!(
                "unsafe ipc parent owner, postfix={}, expected uid {expected_uid}, got {owner_uid}: {}",
                postfix,
                parent_dir.display()
            ),
        )
        .into());
    }

    let expected_mode = expected_ipc_parent_mode(postfix);
    // Include special bits (setuid/setgid/sticky) to ensure the directory is hardened to the exact
    // expected mode.
    let current_mode = (st.st_mode as u32) & 0o7777;
    let repaired_parent_mode = current_mode != expected_mode;
    let had_untrusted_parent_mode = (current_mode & 0o022) != 0;
    if repaired_parent_mode {
        // Use fchmod on the opened fd to avoid path-race between check and chmod.
        if unsafe { hbb_common::libc::fchmod(fd, expected_mode as hbb_common::libc::mode_t) } != 0 {
            let os_err = std::io::Error::last_os_error();
            return Err(Error::new(
                os_err.kind(),
                format!(
                    "failed to chmod ipc parent dir: postfix={}, parent={}, err={}",
                    postfix,
                    parent_dir.display(),
                    os_err
                ),
            )
            .into());
        }
    }
    let should_scrub =
        repaired_parent_mode || adopted_foreign_service_parent || had_untrusted_parent_mode;
    Ok(should_scrub)
}

pub(crate) fn scrub_secure_ipc_parent_dir(path: &str, postfix: &str) -> ResultType<()> {
    let parent_dir = Path::new(path)
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, format!("invalid ipc path: {path}")))?;
    let parent_c = CString::new(parent_dir.as_os_str().as_bytes().to_vec())?;
    let fd = open_ipc_parent_dir_fd(&parent_c).map_err(|err| {
        Error::new(
            err.kind(),
            format!(
                "failed to open ipc parent dir for scrub (no-follow): postfix={}, parent={}, err={}",
                postfix,
                parent_dir.display(),
                err
            ),
        )
    })?;
    let _fd_guard = FdGuard(fd);
    scrub_preexisting_ipc_parent_entries(fd, parent_dir, postfix)
}
