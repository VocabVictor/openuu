use super::*;

// Pins the HELPER's contract, which is all `new_drm_listener` consists of at that line -- not
// the call site itself. Binding the real `/tmp/<app>-service/ipc_drm` from a test would collide
// with a live root service, so "the listener still calls this" is not covered here.
#[test]
pub(super) fn test_remove_ipc_entry_via_secure_parent_fd_clears_an_empty_directory_squatter() {
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

    super::super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref()).unwrap();
    assert!(
        !squatter.exists(),
        "the fd-based removal picks AT_REMOVEDIR and clears it"
    );

    // Idempotent: this runs before every bind, so a path that is already gone is not an error.
    super::super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref()).unwrap();

    // The ORDINARY case, and the one the listener hits on every restart: a stale socket left by
    // the previous run, i.e. a regular file. Covered here because the other file-removal test
    // goes through `remove_parent_entry_via_fd` and the postfix path, not this entry point.
    std::fs::write(&squatter, b"stale").unwrap();
    super::super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref()).unwrap();
    assert!(!squatter.exists(), "a stale regular file is cleared too");

    // And the documented limit, pinned so the doc cannot drift: AT_REMOVEDIR is rmdir(2), so a
    // NON-empty squatter is reported, not cleared. The caller logs that and carries on; nothing
    // here should ever start deleting a tree it did not create.
    std::fs::create_dir(&squatter).unwrap();
    std::fs::write(squatter.join("planted"), b"x").unwrap();
    assert!(
        super::super::remove_ipc_entry_via_secure_parent_fd(squatter.to_string_lossy().as_ref())
            .is_err(),
        "a non-empty directory must be reported, not silently left as success"
    );
    assert!(squatter.join("planted").exists(), "and not deleted");

    std::fs::remove_dir_all(&base).ok();
}

#[test]
pub(super) fn test_write_pid_file_rejects_symlink() {
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

    let res = super::super::write_pid_file(&link);
    assert!(res.is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "origin");

    std::fs::remove_file(&link).ok();
    std::fs::remove_file(&target).ok();
    std::fs::remove_dir_all(&base).ok();
}

#[test]
pub(super) fn test_ensure_secure_ipc_parent_dir_rejects_symlink_parent() {
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
        super::super::ensure_secure_ipc_parent_dir(ipc_path.to_string_lossy().as_ref(), "_service");
    assert!(res.is_err());
    std::fs::remove_file(&link_dir).ok();
    std::fs::remove_dir_all(&real_dir).ok();
    std::fs::remove_dir_all(&base).ok();
}

#[test]
pub(super) fn test_ensure_secure_ipc_parent_dir_creates_parent_with_expected_mode() {
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

    let res = super::super::ensure_secure_ipc_parent_dir(ipc_path.to_string_lossy().as_ref(), "");
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
pub(super) fn test_scrub_preexisting_ipc_parent_entries_only_removes_target_postfix_artifacts() {
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
    let base_fd = super::super::open_ipc_parent_dir_fd(&base_c).unwrap();
    let _base_guard = super::super::FdGuard(base_fd);
    super::super::scrub_preexisting_ipc_parent_entries(base_fd, &base, "_service").unwrap();

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
