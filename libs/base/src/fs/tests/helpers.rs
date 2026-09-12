use super::*;

pub(super) struct TestTempDir {
    pub(super) path: PathBuf,
}

impl TestTempDir {
    pub(super) fn new(prefix: &str) -> Self {
        Self {
            path: unique_temp_dir(prefix),
        }
    }

    pub(super) fn join(&self, path: &str) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub(super) fn unique_temp_dir(prefix: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), timestamp))
}

pub(super) fn new_file_entry(name: &str) -> FileEntry {
    let mut entry = FileEntry::new();
    entry.name = name.to_string();
    entry
}

pub(super) fn new_validation_job(id: i32) -> TransferJob {
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

pub(super) fn new_write_job(id: i32, download_dir: PathBuf, name: &str) -> ResultType<TransferJob> {
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

pub(super) fn assert_err_contains(err: anyhow::Error, expected: &str) {
    assert!(
        err.to_string().contains(expected),
        "expected error containing '{}', got: {}",
        expected,
        err
    );
}
