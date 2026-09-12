use super::*;

#[test]
fn test_portable_service_shmem_dir_acl_policy() {
    let dir = unique_acl_test_path("dir");
    fs::create_dir_all(&dir).unwrap();
    set_path_permission_for_portable_service_shmem_dir(&dir).unwrap();

    let (dacl, sd_guard) = get_file_dacl(&dir).unwrap();
    let current_user_sid =
        sid_string_to_local_alloc_guard(&current_process_user_sid_string().unwrap()).unwrap();
    let system_sid = sid_string_to_local_alloc_guard("S-1-5-18").unwrap();
    let admin_sid = sid_string_to_local_alloc_guard("S-1-5-32-544").unwrap();
    let auth_users_sid = sid_string_to_local_alloc_guard("S-1-5-11").unwrap();
    let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
    let users_sid = sid_string_to_local_alloc_guard("S-1-5-32-545").unwrap();

    assert!(has_allow_ace_with_mask(
        dacl,
        system_sid.as_sid_ptr(),
        FILE_ALL_ACCESS.0
    ));
    assert!(has_allow_ace_with_mask(
        dacl,
        admin_sid.as_sid_ptr(),
        FILE_ALL_ACCESS.0
    ));
    assert!(has_allow_ace_with_mask(
        dacl,
        current_user_sid.as_sid_ptr(),
        FILE_ALL_ACCESS.0
    ));
    assert!(has_allow_ace_with_mask(
        dacl,
        auth_users_sid.as_sid_ptr(),
        FILE_GENERIC_WRITE.0
    ));
    assert!(!has_any_allow_ace_for_sid(dacl, everyone_sid.as_sid_ptr()));
    assert!(!has_any_allow_ace_for_sid(dacl, users_sid.as_sid_ptr()));
    assert!(is_dacl_protected(PSECURITY_DESCRIPTOR(
        sd_guard.as_sid_ptr()
    )));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_portable_service_shmem_file_acl_policy() {
    let dir = unique_acl_test_path("file");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("shared_memory_portable_service_test");
    fs::write(&file, b"x").unwrap();
    set_path_permission_for_portable_service_shmem_file(&file).unwrap();

    let (dacl, sd_guard) = get_file_dacl(&file).unwrap();
    let current_user_sid =
        sid_string_to_local_alloc_guard(&current_process_user_sid_string().unwrap()).unwrap();
    let system_sid = sid_string_to_local_alloc_guard("S-1-5-18").unwrap();
    let admin_sid = sid_string_to_local_alloc_guard("S-1-5-32-544").unwrap();
    let auth_users_sid = sid_string_to_local_alloc_guard("S-1-5-11").unwrap();
    let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
    let users_sid = sid_string_to_local_alloc_guard("S-1-5-32-545").unwrap();

    assert!(has_allow_ace_with_mask(
        dacl,
        system_sid.as_sid_ptr(),
        FILE_ALL_ACCESS.0
    ));
    assert!(has_allow_ace_with_mask(
        dacl,
        admin_sid.as_sid_ptr(),
        FILE_ALL_ACCESS.0
    ));
    assert!(has_allow_ace_with_mask(
        dacl,
        current_user_sid.as_sid_ptr(),
        FILE_ALL_ACCESS.0
    ));
    assert!(!has_any_allow_ace_for_sid(
        dacl,
        auth_users_sid.as_sid_ptr()
    ));
    assert!(!has_any_allow_ace_for_sid(dacl, everyone_sid.as_sid_ptr()));
    assert!(!has_any_allow_ace_for_sid(dacl, users_sid.as_sid_ptr()));
    assert!(is_dacl_protected(PSECURITY_DESCRIPTOR(
        sd_guard.as_sid_ptr()
    )));

    let _ = fs::remove_file(&file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_set_path_permission_rx_applies_recursively() {
    let root = unique_acl_test_path("set_path_permission");
    let child_dir = root.join("child");
    let child_file = child_dir.join("helper.exe");
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(&child_file, b"x").unwrap();

    if let Err(err) = set_path_permission(&root, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0) {
        let text = err.to_string();
        let _ = fs::remove_file(&child_file);
        let _ = fs::remove_dir_all(&root);
        if text.contains("win32_error=5") || text.contains("Access is denied") {
            eprintln!(
                "skip test_set_path_permission_rx_applies_recursively: insufficient WRITE_DAC in current environment: {}",
                text
            );
            return;
        }
        panic!("set_path_permission failed unexpectedly: {}", text);
    }

    let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
    let rx_mask = FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0;
    for target in [&root, &child_dir, &child_file] {
        let (dacl, _sd_guard) = get_file_dacl(target).unwrap();
        assert!(
            has_allow_ace_with_mask(dacl, everyone_sid.as_sid_ptr(), rx_mask),
            "Everyone RX grant missing on '{}'",
            target.display()
        );
    }

    let _ = fs::remove_file(&child_file);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_portable_service_shmem_dir_acl_rejects_file_target() {
    let dir = unique_acl_test_path("dir_target_file");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("target.txt");
    fs::write(&file, b"x").unwrap();
    let result = set_path_permission_for_portable_service_shmem_dir(&file);
    assert!(result.is_err());
    let _ = fs::remove_file(&file);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_portable_service_shmem_file_acl_rejects_dir_target() {
    let dir = unique_acl_test_path("file_target_dir");
    fs::create_dir_all(&dir).unwrap();
    let result = set_path_permission_for_portable_service_shmem_file(&dir);
    assert!(result.is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_portable_service_shmem_file_acl_rejects_missing_target() {
    let path = unique_acl_test_path("missing").join("shared_memory_missing");
    let result = set_path_permission_for_portable_service_shmem_file(&path);
    assert!(result.is_err());
}
