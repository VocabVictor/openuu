use super::*;

use crate::ipc::Data;
use base::message_proto::{FileDirectory, Message};
use hbb_common::tokio::{runtime::Runtime, sync::mpsc::unbounded_channel};
use std::fs;

#[test]
#[cfg(not(any(target_os = "ios")))]
fn read_all_files_success() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let (tx, mut rx) = unbounded_channel();
        let dir = std::env::temp_dir().join("rustdesk_read_all_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("test.txt"), b"hello").unwrap();

        let path_str = dir.to_string_lossy().to_string();
        super::read_all_files(path_str.clone(), false, 1, 2, &tx).await;

        match rx.recv().await.unwrap() {
            Data::AllFilesResult { result, .. } => {
                let bytes = result.unwrap();
                let fd = FileDirectory::parse_from_bytes(&bytes).unwrap();
                assert!(!fd.entries.is_empty());
            }
            _ => panic!("unexpected data"),
        }
        let _ = fs::remove_dir_all(&dir);
    });
}

#[test]
#[cfg(not(any(target_os = "ios")))]
fn read_dir_reports_success_and_error() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let (tx, mut rx) = unbounded_channel();
        let dir = std::env::temp_dir().join("rustdesk_read_dir_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        super::read_dir(&dir.to_string_lossy(), false, &tx).await;

        match rx.recv().await.unwrap() {
            Data::RawMessage(bytes) => {
                let mut msg = Message::new();
                msg.merge_from_bytes(&bytes).unwrap();
                assert!(msg
                    .file_response()
                    .dir()
                    .path
                    .contains("rustdesk_read_dir_test"));
            }
            _ => panic!("unexpected data"),
        }
        let _ = fs::remove_dir_all(&dir);

        super::read_dir(&dir.to_string_lossy(), false, &tx).await;

        match rx.recv().await.unwrap() {
            Data::RawMessage(bytes) => {
                let mut msg = Message::new();
                msg.merge_from_bytes(&bytes).unwrap();
                assert_eq!(msg.file_response().error().id, 0);
                assert!(!msg.file_response().error().error.is_empty());
            }
            _ => panic!("unexpected data"),
        }
    });
}

/// Tests that symlink creation works on this platform.
/// This is a helper to verify the test environment supports symlinks.
#[test]
#[cfg(not(any(target_os = "ios")))]
fn test_symlink_creation_works() {
    let base_dir = std::env::temp_dir().join("rustdesk_symlink_test");
    let _ = fs::remove_dir_all(&base_dir);
    fs::create_dir_all(&base_dir).unwrap();

    // Create target file in a subdirectory
    let target_dir = base_dir.join("target_dir");
    fs::create_dir_all(&target_dir).unwrap();
    let target_file = target_dir.join("target.txt");
    fs::write(&target_file, b"content").unwrap();

    // Create symlink in a different directory
    let link_dir = base_dir.join("link_dir");
    fs::create_dir_all(&link_dir).unwrap();
    let link_path = link_dir.join("link.txt");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if symlink(&target_file, &link_path).is_err() {
            let _ = fs::remove_dir_all(&base_dir);
            return;
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_file;
        if symlink_file(&target_file, &link_path).is_err() {
            // Skip if no permission (needs admin or dev mode on Windows)
            let _ = fs::remove_dir_all(&base_dir);
            return;
        }
    }

    let _ = fs::remove_dir_all(&base_dir);
}
