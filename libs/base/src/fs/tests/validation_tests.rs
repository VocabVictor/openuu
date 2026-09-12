use super::*;

#[test]
pub(super) fn path_traversal_e2e_write_rejects_relative_escape() {
    let tmp_root = TestTempDir::new("rustdesk_e2e_relative");
    let downloads = tmp_root.join("downloads");
    std::fs::create_dir_all(&downloads).expect("create downloads dir");

    let err = new_write_job(1, downloads, "../traversal_proof.txt")
        .expect_err("relative path traversal must be rejected");
    assert_err_contains(err, "path traversal");
    assert!(!tmp_root.join("traversal_proof.txt").exists());
}

#[test]
pub(super) fn path_traversal_e2e_write_rejects_absolute_path() {
    let tmp_root = TestTempDir::new("rustdesk_e2e_absolute");
    let downloads = tmp_root.join("downloads");
    let absolute_target = tmp_root.join("fake_ssh").join("authorized_keys");
    std::fs::create_dir_all(&downloads).expect("create downloads dir");

    let err = new_write_job(2, downloads, &absolute_target.to_string_lossy())
        .expect_err("absolute path must be rejected");
    assert_err_contains(err, "absolute path");
    assert!(!absolute_target.exists());
}

#[test]
#[cfg_attr(windows, ignore = "requires symlink privilege to create test symlink")]
pub(super) fn path_traversal_e2e_write_rejects_symlink_escape() {
    let tmp_root = TestTempDir::new("rustdesk_e2e_symlink");
    let downloads = tmp_root.join("downloads");
    let outside = tmp_root.join("outside");
    let escaped_target = outside.join("escape.txt");
    std::fs::create_dir_all(&downloads).expect("create downloads dir");
    std::fs::create_dir_all(&outside).expect("create outside dir");

    let symlink_path = downloads.join("link");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(&outside, &symlink_path).expect("create symlink for test");
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_dir;
        symlink_dir(&outside, &symlink_path).expect("create directory symlink for test");
    }

    let err = new_write_job(3, downloads, "link/escape.txt")
        .expect_err("symlink traversal must be rejected");
    assert_err_contains(err, "symlink");
    assert!(!escaped_target.exists());
}

#[test]
pub(super) fn set_files_allows_single_empty_name_for_single_file_transfer() {
    let mut job = new_validation_job(101);
    assert!(job.set_files(vec![new_file_entry("")]).is_ok());
}

#[test]
pub(super) fn set_files_rejects_empty_name_in_multi_file_transfer() {
    let mut job = new_validation_job(102);
    let err = job
        .set_files(vec![new_file_entry(""), new_file_entry("ok.txt")])
        .expect_err("empty name in multi-file transfer must be rejected");
    assert_err_contains(err, "empty file name");
}

#[test]
pub(super) fn set_files_rejects_null_byte_name() {
    let mut job = new_validation_job(103);
    let err = job
        .set_files(vec![new_file_entry("bad\0name.txt")])
        .expect_err("null byte in file name must be rejected");
    assert_err_contains(err, "null bytes");
}

#[test]
pub(super) fn set_files_rejects_mixed_entries_when_one_is_traversal() {
    let mut job = new_validation_job(104);
    let err = job
        .set_files(vec![
            new_file_entry("safe/file.txt"),
            new_file_entry("../../escape.txt"),
        ])
        .expect_err("any traversal entry must reject the full file list");
    assert_err_contains(err, "path traversal");
}

#[cfg(windows)]
#[test]
pub(super) fn set_files_rejects_unc_absolute_path() {
    let mut job = new_validation_job(105);
    let err = job
        .set_files(vec![new_file_entry("\\\\server\\share\\payload.txt")])
        .expect_err("UNC absolute path must be rejected");
    assert_err_contains(err, "absolute path");
}

#[cfg(not(windows))]
#[test]
pub(super) fn set_files_allows_backslash_prefixed_name_on_unix() {
    let mut job = new_validation_job(105);
    assert!(job
        .set_files(vec![new_file_entry("\\\\server\\share\\payload.txt")])
        .is_ok());
}

#[test]
pub(super) fn remove_file_rejects_empty_path() {
    let err = remove_file("").expect_err("empty file path must be rejected");
    assert_err_contains(err, "cannot be empty");
}

#[test]
pub(super) fn remove_file_rejects_null_byte_path() {
    let err = remove_file("bad\0path").expect_err("null byte path must be rejected");
    assert_err_contains(err, "null bytes");
}

#[test]
pub(super) fn create_dir_rejects_empty_path() {
    let err = create_dir("").expect_err("empty directory path must be rejected");
    assert_err_contains(err, "cannot be empty");
}

#[test]
pub(super) fn create_dir_rejects_null_byte_path() {
    let err = create_dir("bad\0path").expect_err("null byte path must be rejected");
    assert_err_contains(err, "null bytes");
}

#[test]
pub(super) fn rename_file_rejects_invalid_new_name() {
    let tmp_root = TestTempDir::new("rustdesk_rename_invalid");
    let src = tmp_root.join("source.txt");
    std::fs::create_dir_all(&tmp_root.path).expect("create temp dir");
    std::fs::write(&src, b"content").expect("create source file");

    let src_str = src.to_string_lossy().to_string();

    let err_empty =
        rename_file(&src_str, "").expect_err("empty new file name must be rejected");
    assert_err_contains(err_empty, "cannot be empty");

    let err_traversal = rename_file(&src_str, "../escape.txt")
        .expect_err("traversal new file name must be rejected");
    assert_err_contains(err_traversal, "path traversal");

    let err_null = rename_file(&src_str, "bad\0name.txt")
        .expect_err("null byte in new file name must be rejected");
    assert_err_contains(err_null, "null bytes");

    #[cfg(windows)]
    {
        let err_abs = rename_file(&src_str, "C:\\Windows\\Temp\\payload.txt")
            .expect_err("absolute new file name must be rejected");
        assert_err_contains(err_abs, "absolute path");
    }
    #[cfg(not(windows))]
    {
        let err_abs = rename_file(&src_str, "/tmp/payload.txt")
            .expect_err("absolute new file name must be rejected");
        assert_err_contains(err_abs, "absolute path");
    }
}

#[test]
pub(super) fn rename_file_accepts_valid_new_name() {
    let tmp_root = TestTempDir::new("rustdesk_rename_ok");
    let src = tmp_root.join("rename_src.txt");
    let dst = tmp_root.join("renamed.txt");
    std::fs::create_dir_all(&tmp_root.path).expect("create temp dir");
    std::fs::write(&src, b"content").expect("create source file");

    let src_str = src.to_string_lossy().to_string();
    rename_file(&src_str, "renamed.txt").expect("rename should succeed");

    assert!(!src.exists());
    assert!(dst.exists());
}

#[cfg(windows)]
#[test]
pub(super) fn set_files_rejects_windows_drive_absolute_path() {
    let mut job = new_validation_job(106);
    let err = job
        .set_files(vec![new_file_entry("C:\\Windows\\Temp\\payload.txt")])
        .expect_err("drive-letter absolute path must be rejected");
    assert_err_contains(err, "absolute path");
}

#[cfg(windows)]
#[test]
pub(super) fn set_files_rejects_windows_verbatim_drive_absolute_path() {
    let mut job = new_validation_job(1061);
    let err = job
        .set_files(vec![new_file_entry(r"\\?\C:\Windows\Temp\x.txt")])
        .expect_err("verbatim drive absolute path must be rejected");
    assert_err_contains(err, "absolute path");
}
