use super::*;

#[test]
fn test_set_path_permission_rejects_reparse_entrypoint() {
    let root = unique_acl_test_path("reparse_entry");
    let real_dir = root.join("real");
    let link_dir = root.join("link");
    fs::create_dir_all(&real_dir).unwrap();
    if !try_create_dir_reparse_point(
        &real_dir,
        &link_dir,
        "test_set_path_permission_rejects_reparse_entrypoint",
    ) {
        let _ = fs::remove_dir_all(&real_dir);
        let _ = fs::remove_dir_all(&root);
        return;
    }

    let result = set_path_permission(&link_dir, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0);
    let text = result.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        text.contains("reparse point"),
        "expected reparse-point rejection, got '{}'",
        text
    );

    let _ = fs::remove_dir(&link_dir);
    let _ = fs::remove_dir_all(&real_dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_portable_service_shmem_dir_acl_rejects_reparse_target() {
    let root = unique_acl_test_path("reparse_shmem_dir");
    let real_dir = root.join("real");
    let link_dir = root.join("link");
    fs::create_dir_all(&real_dir).unwrap();
    if !try_create_dir_reparse_point(
        &real_dir,
        &link_dir,
        "test_portable_service_shmem_dir_acl_rejects_reparse_target",
    ) {
        let _ = fs::remove_dir_all(&real_dir);
        let _ = fs::remove_dir_all(&root);
        return;
    }

    let result = set_path_permission_for_portable_service_shmem_dir(&link_dir);
    let text = result.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        text.contains("reparse point"),
        "expected reparse-point rejection, got '{}'",
        text
    );

    let _ = fs::remove_dir(&link_dir);
    let _ = fs::remove_dir_all(&real_dir);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_portable_service_shmem_file_acl_rejects_reparse_target() {
    let root = unique_acl_test_path("reparse_shmem_file");
    let real_file = root.join("real.txt");
    let link_file = root.join("link.txt");
    fs::create_dir_all(&root).unwrap();
    fs::write(&real_file, b"x").unwrap();
    if !try_create_file_reparse_point(
        &real_file,
        &link_file,
        "test_portable_service_shmem_file_acl_rejects_reparse_target",
    ) {
        let _ = fs::remove_file(&real_file);
        let _ = fs::remove_dir_all(&root);
        return;
    }

    let result = set_path_permission_for_portable_service_shmem_file(&link_file);
    let text = result.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        text.contains("reparse point"),
        "expected reparse-point rejection, got '{}'",
        text
    );

    let _ = fs::remove_file(&link_file);
    let _ = fs::remove_file(&real_file);
    let _ = fs::remove_dir_all(&root);
}
