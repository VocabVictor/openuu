use super::*;

#[test]
pub(super) fn test_scrub_preexisting_ipc_parent_entries_should_bind_to_opened_inode_not_path() {
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
    let trusted_parent_fd = super::super::open_ipc_parent_dir_fd(&trusted_parent_c).unwrap();
    let _trusted_parent_guard = super::super::FdGuard(trusted_parent_fd);

    // Swap the path after the trusted inode has been opened.
    std::fs::rename(&trusted_parent, &trusted_parent_moved).unwrap();
    std::fs::rename(&attacker_parent, &trusted_parent).unwrap();

    super::super::scrub_preexisting_ipc_parent_entries(trusted_parent_fd, &trusted_parent, "_service")
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
pub(super) fn test_ensure_secure_ipc_parent_dir_keeps_service_artifacts_before_liveness_probe() {
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
        super::super::ensure_secure_ipc_parent_dir(ipc_file.to_string_lossy().as_ref(), "_service");
    assert_eq!(res.unwrap(), true);

    // Parent hardening should run first; artifacts should stay until liveness probe completes.
    assert!(ipc_file.exists(), "ipc socket marker should be preserved");
    assert!(ipc_pid_file.exists(), "pid marker should be preserved");

    std::fs::remove_dir_all(&base).ok();
}

#[test]
pub(super) fn test_ensure_secure_ipc_parent_dir_marks_non_service_mode_repair_for_scrub() {
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

    let res = super::super::ensure_secure_ipc_parent_dir(ipc_file.to_string_lossy().as_ref(), "");
    assert_eq!(res.unwrap(), true);

    std::fs::remove_dir_all(&base).ok();
}

#[test]
pub(super) fn test_should_scrub_parent_entries_after_check_pid_only_when_requested_and_not_alive() {
    assert!(!super::super::should_scrub_parent_entries_after_check_pid(
        false, false
    ));
    assert!(!super::super::should_scrub_parent_entries_after_check_pid(
        false, true
    ));
    assert!(super::super::should_scrub_parent_entries_after_check_pid(
        true, false
    ));
    assert!(!super::super::should_scrub_parent_entries_after_check_pid(
        true, true
    ));
}
