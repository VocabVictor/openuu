#[cfg(windows)]
use std::os::windows::prelude::*;
use std::{
    fmt::{Debug, Display},
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufStream as TokioBufStream},
};

use crate::message_proto::*;
// https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html
use hbb_common::{
    anyhow::anyhow,
    bail,
    compress::{compress, decompress},
    config::Config,
    get_version_number, ResultType, Stream,
};

static NEXT_JOB_ID: AtomicI32 = AtomicI32::new(1);

mod dir_read;
pub use dir_read::*;
mod job_types;
pub use job_types::*;
mod job_structs;
pub use job_structs::*;
mod validation;
pub use validation::*;
mod job_new;
mod job_write;
mod job_read;
mod job_state;
mod messages;
pub use messages::*;
mod jobs;
pub use jobs::*;
mod file_ops;
pub use file_ops::*;

#[cfg(test)]
#[path = "fs_transfer_tests.rs"]
mod transfer_network_tests;

pub fn get_next_job_id() -> i32 {
    NEXT_JOB_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn update_next_job_id(id: i32) {
    NEXT_JOB_ID.store(id, Ordering::SeqCst);
}

impl TransferJob {
}

#[cfg(test)]
mod tests {
    use super::*;
    use protobuf::Message as _;

    #[test]
    fn obsolete_print_jobs_cannot_read_files() {
        let result = TransferJob::new_read(
            1, JobType::Printer, String::new(),
            DataSource::MemoryCursor(Cursor::new(vec![1, 2, 3])),
            0, false, false, false,
        );
        assert!(matches!(result, Err(e) if e.to_string() == "Unsupported transfer type"));
    }

    #[tokio::test]
    async fn obsolete_print_jobs_cannot_write_files() {
        let dir = TestTempDir::new("openuu_obsolete_print");
        let mut job = TransferJob::new_write(
            1, JobType::Printer, String::new(),
            DataSource::FilePath(dir.path.clone()), 0, false, true, false,
        );
        let result = job.write(FileTransferBlock { id: 1, ..Default::default() }).await;
        assert!(matches!(result, Err(e) if e.to_string() == "Unsupported transfer type"));
        assert!(!dir.path.exists());
    }

    #[test]
    fn adaptive_compression_recovers_after_incompressible_data() {
        let mut policy = TransferCompression::default();
        let mut seed = 123456789u32;
        let noise: Vec<u8> = (0..128 * 1024).map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as u8
        }).collect();
        assert!(policy.encode(&noise).is_none());
        let text = vec![b'a'; 128 * 1024];
        for _ in 0..31 {
            assert!(policy.encode(&text).is_none());
        }
        let encoded = policy.encode(&text).unwrap();
        assert_eq!(decompress(&encoded), text);
        assert!(is_compressed_file("archive.ZIP"));
    }

    #[tokio::test]
    async fn batched_transfer_preserves_payload_and_completion() {
        let dir = TestTempDir::new("openuu_transfer_batch");
        std::fs::create_dir_all(&dir.path).unwrap();
        let data: Vec<u8> = (0..1024 * 1024 + 37).map(|i| (i % 251) as u8).collect();
        let path = dir.join("payload.bin");
        std::fs::write(&path, &data).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (receiver, peer) = listener.accept().await.unwrap();
        let mut sender = hbb_common::Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, addr));
        let receive = tokio::spawn(async move {
            let mut receiver = hbb_common::tcp::FramedStream::from(receiver, peer);
            let mut actual = Vec::new();
            loop {
                let bytes = receiver.next().await.unwrap().unwrap();
                let msg = Message::parse_from_bytes(&bytes).unwrap();
                if let Some(message::Union::FileResponse(response)) = msg.union {
                    match response.union {
                        Some(file_response::Union::Block(block)) => {
                            if block.compressed { actual.extend(decompress(&block.data)); }
                            else { actual.extend_from_slice(&block.data); }
                        }
                        Some(file_response::Union::Done(_)) => return actual,
                        Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                        _ => {}
                    }
                }
            }
        });
        let job = TransferJob::new_read(7, JobType::Generic, String::new(),
            DataSource::FilePath(path), 0, false, false, false).unwrap();
        let mut jobs = vec![job];
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !jobs.is_empty() { handle_read_jobs(&mut jobs, &mut sender).await.unwrap(); }
            assert_eq!(receive.await.unwrap(), data);
        }).await.unwrap();
    }

    #[tokio::test]
    async fn small_files_batch_preserves_empty_files_and_order() {
        let dir = TestTempDir::new("openuu_small_files");
        std::fs::create_dir_all(&dir.path).unwrap();
        for i in 0..300 {
            let folder = dir.join(&format!("group{}", i % 3));
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(format!("file{i}.bin")), vec![(i % 251) as u8; if i % 10 == 0 { 0 } else { 1024 }]).unwrap();
        }
        let job = TransferJob::new_read(7, JobType::Generic, String::new(),
            DataSource::FilePath(dir.path.clone()), 0, false, false, false).unwrap();
        let mut data = Vec::new();
        for entry in job.files() { data.extend(std::fs::read(dir.path.join(&entry.name)).unwrap()); }
        let file_count = job.files().len();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (receiver, peer) = listener.accept().await.unwrap();
        let mut sender = hbb_common::Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, addr));
        let receive = tokio::spawn(async move {
            let mut receiver = hbb_common::tcp::FramedStream::from(receiver, peer);
            let mut actual = Vec::new();
            let mut ended = std::collections::HashSet::new();
            loop {
                let bytes = receiver.next().await.unwrap().unwrap();
                let msg = Message::parse_from_bytes(&bytes).unwrap();
                if let Some(message::Union::FileResponse(response)) = msg.union {
                    match response.union {
                        Some(file_response::Union::Block(block)) => {
                            if block.data.is_empty() { assert!(ended.insert(block.file_num)); }
                            if block.compressed { actual.extend(decompress(&block.data)); }
                            else { actual.extend_from_slice(&block.data); }
                        }
                        Some(file_response::Union::Done(_)) => { assert_eq!(ended.len(), file_count); return actual; },
                        Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                        _ => {}
                    }
                }
            }
        });
        let mut jobs = vec![job];
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let mut ticks = 0;
            let start = std::time::Instant::now();
            while !jobs.is_empty() {
                handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
                ticks += 1;
            }
            println!("small files: {file_count}, scheduler rounds: {ticks}, elapsed: {:?}", start.elapsed());
            assert_eq!(receive.await.unwrap(), data);
        }).await.unwrap();
    }

    #[tokio::test]
    async fn paused_transfer_preserves_offset_and_payload() {
        let dir = TestTempDir::new("openuu_transfer_pause");
        std::fs::create_dir_all(&dir.path).unwrap();
        let data: Vec<u8> = (0..8 * 1024 * 1024 + 37).map(|i| (i % 251) as u8).collect();
        let path = dir.join("payload.bin");
        std::fs::write(&path, &data).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (receiver, peer) = listener.accept().await.unwrap();
        let mut sender = hbb_common::Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, addr));
        let receive = tokio::spawn(async move {
            let mut receiver = hbb_common::tcp::FramedStream::from(receiver, peer);
            let mut actual = Vec::new();
            loop {
                let bytes = receiver.next().await.unwrap().unwrap();
                let msg = Message::parse_from_bytes(&bytes).unwrap();
                if let Some(message::Union::FileResponse(response)) = msg.union {
                    match response.union {
                        Some(file_response::Union::Block(block)) => {
                            if block.compressed { actual.extend(decompress(&block.data)); }
                            else { actual.extend_from_slice(&block.data); }
                        }
                        Some(file_response::Union::Done(_)) => return actual,
                        Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                        _ => {}
                    }
                }
            }
        });
        let job = TransferJob::new_read(7, JobType::Generic, String::new(),
            DataSource::FilePath(path), 0, false, false, false).unwrap();
        let mut jobs = vec![job];
        jobs[0].paused = true;
        handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
        assert_eq!(jobs[0].finished_size(), 0);
        assert!(jobs[0].data_stream.is_none());
        jobs[0].paused = false;
        handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
        assert!(!jobs.is_empty());
        let offset = jobs[0].finished_size();
        assert!(offset > 0);
        jobs[0].paused = true;
        for _ in 0..5 { handle_read_jobs(&mut jobs, &mut sender).await.unwrap(); }
        assert_eq!(jobs[0].finished_size(), offset);
        assert!(jobs[0].data_stream.is_some());
        jobs[0].paused = false;
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !jobs.is_empty() { handle_read_jobs(&mut jobs, &mut sender).await.unwrap(); }
            assert_eq!(receive.await.unwrap(), data);
        }).await.unwrap();
    }

    #[tokio::test]
    async fn small_file_batch_still_requires_each_overwrite_confirmation() {
        let dir = TestTempDir::new("openuu_small_file_confirm");
        std::fs::create_dir_all(&dir.path).unwrap();
        std::fs::write(dir.join("a.bin"), b"first").unwrap();
        std::fs::write(dir.join("b.bin"), b"second").unwrap();
        let mut job = TransferJob::new_read(77, JobType::Generic, String::new(),
            DataSource::FilePath(dir.path.clone()), 0, false, false, true).unwrap();
        for file_num in 0..2 {
            assert!(job.init_data_stream_for_cm().await.unwrap().is_some());
            assert!(job.read().await.unwrap().is_none());
            assert!(!job.job_completed());
            let mut confirm = FileTransferSendConfirmRequest { id: 77, file_num, ..Default::default() };
            confirm.set_skip(false);
            job.confirm(&confirm).await;
            let mut actual = Vec::new();
            loop {
                let block = job.read().await.unwrap().unwrap();
                if block.data.is_empty() { break; }
                if block.compressed { actual.extend(decompress(&block.data)); }
                else { actual.extend_from_slice(&block.data); }
            }
            assert_eq!(actual, std::fs::read(dir.path.join(&job.files()[file_num as usize].name)).unwrap());
        }
    }

    #[tokio::test]
    async fn small_file_buffer_does_not_truncate_a_growing_file() {
        let dir = TestTempDir::new("openuu_small_file_growth");
        std::fs::create_dir_all(&dir.path).unwrap();
        let path = dir.join("growing.bin");
        std::fs::write(&path, b"x").unwrap();
        let mut job = TransferJob::new_read(78, JobType::Generic, String::new(),
            DataSource::FilePath(path.clone()), 0, false, false, false).unwrap();
        std::fs::write(&path, b"expanded contents").unwrap();
        job.init_data_stream_for_cm().await.unwrap();
        let mut actual = Vec::new();
        loop {
            let block = job.read().await.unwrap().unwrap();
            if block.data.is_empty() { break; }
            actual.extend_from_slice(&block.data);
        }
        assert_eq!(actual, b"expanded contents");
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            Self {
                path: unique_temp_dir(prefix),
            }
        }

        fn join(&self, path: &str) -> PathBuf {
            self.path.join(path)
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), timestamp))
    }

    fn new_file_entry(name: &str) -> FileEntry {
        let mut entry = FileEntry::new();
        entry.name = name.to_string();
        entry
    }

    fn new_validation_job(id: i32) -> TransferJob {
        TransferJob::new_write(
            id,
            JobType::Generic,
            "/fake/remote".to_string(),
            DataSource::FilePath(std::env::temp_dir().join(format!("rustdesk_validation_{id}"))),
            0,
            false,
            true,
            false,
        )
    }

    fn new_write_job(id: i32, download_dir: PathBuf, name: &str) -> ResultType<TransferJob> {
        let job = TransferJob::new_write(
            id,
            JobType::Generic,
            "/fake/remote".to_string(),
            DataSource::FilePath(download_dir),
            0,
            false,
            true,
            false,
        )
        .with_files(vec![new_file_entry(name)])?;
        Ok(job)
    }

    fn assert_err_contains(err: anyhow::Error, expected: &str) {
        assert!(
            err.to_string().contains(expected),
            "expected error containing '{}', got: {}",
            expected,
            err
        );
    }

    #[test]
    fn path_traversal_e2e_write_rejects_relative_escape() {
        let tmp_root = TestTempDir::new("rustdesk_e2e_relative");
        let downloads = tmp_root.join("downloads");
        std::fs::create_dir_all(&downloads).expect("create downloads dir");

        let err = new_write_job(1, downloads, "../traversal_proof.txt")
            .expect_err("relative path traversal must be rejected");
        assert_err_contains(err, "path traversal");
        assert!(!tmp_root.join("traversal_proof.txt").exists());
    }

    #[test]
    fn path_traversal_e2e_write_rejects_absolute_path() {
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
    fn path_traversal_e2e_write_rejects_symlink_escape() {
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
    fn set_files_allows_single_empty_name_for_single_file_transfer() {
        let mut job = new_validation_job(101);
        assert!(job.set_files(vec![new_file_entry("")]).is_ok());
    }

    #[test]
    fn set_files_rejects_empty_name_in_multi_file_transfer() {
        let mut job = new_validation_job(102);
        let err = job
            .set_files(vec![new_file_entry(""), new_file_entry("ok.txt")])
            .expect_err("empty name in multi-file transfer must be rejected");
        assert_err_contains(err, "empty file name");
    }

    #[test]
    fn set_files_rejects_null_byte_name() {
        let mut job = new_validation_job(103);
        let err = job
            .set_files(vec![new_file_entry("bad\0name.txt")])
            .expect_err("null byte in file name must be rejected");
        assert_err_contains(err, "null bytes");
    }

    #[test]
    fn set_files_rejects_mixed_entries_when_one_is_traversal() {
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
    fn set_files_rejects_unc_absolute_path() {
        let mut job = new_validation_job(105);
        let err = job
            .set_files(vec![new_file_entry("\\\\server\\share\\payload.txt")])
            .expect_err("UNC absolute path must be rejected");
        assert_err_contains(err, "absolute path");
    }

    #[cfg(not(windows))]
    #[test]
    fn set_files_allows_backslash_prefixed_name_on_unix() {
        let mut job = new_validation_job(105);
        assert!(job
            .set_files(vec![new_file_entry("\\\\server\\share\\payload.txt")])
            .is_ok());
    }

    #[test]
    fn remove_file_rejects_empty_path() {
        let err = remove_file("").expect_err("empty file path must be rejected");
        assert_err_contains(err, "cannot be empty");
    }

    #[test]
    fn remove_file_rejects_null_byte_path() {
        let err = remove_file("bad\0path").expect_err("null byte path must be rejected");
        assert_err_contains(err, "null bytes");
    }

    #[test]
    fn create_dir_rejects_empty_path() {
        let err = create_dir("").expect_err("empty directory path must be rejected");
        assert_err_contains(err, "cannot be empty");
    }

    #[test]
    fn create_dir_rejects_null_byte_path() {
        let err = create_dir("bad\0path").expect_err("null byte path must be rejected");
        assert_err_contains(err, "null bytes");
    }

    #[test]
    fn rename_file_rejects_invalid_new_name() {
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
    fn rename_file_accepts_valid_new_name() {
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
    fn set_files_rejects_windows_drive_absolute_path() {
        let mut job = new_validation_job(106);
        let err = job
            .set_files(vec![new_file_entry("C:\\Windows\\Temp\\payload.txt")])
            .expect_err("drive-letter absolute path must be rejected");
        assert_err_contains(err, "absolute path");
    }

    #[cfg(windows)]
    #[test]
    fn set_files_rejects_windows_verbatim_drive_absolute_path() {
        let mut job = new_validation_job(1061);
        let err = job
            .set_files(vec![new_file_entry(r"\\?\C:\Windows\Temp\x.txt")])
            .expect_err("verbatim drive absolute path must be rejected");
        assert_err_contains(err, "absolute path");
    }
}
