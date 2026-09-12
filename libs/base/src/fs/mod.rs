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

#[cfg(test)]
#[path = "fs_transfer_tests.rs"]
mod transfer_network_tests;

pub fn get_next_job_id() -> i32 {
    NEXT_JOB_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn update_next_job_id(id: i32) {
    NEXT_JOB_ID.store(id, Ordering::SeqCst);
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct FileDigest {
    pub size: u64,
    pub modified: u64,
}

#[derive(Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransferJob {
    pub id: i32,
    pub r#type: JobType,
    pub remote: String,
    pub data_source: DataSource,
    pub show_hidden: bool,
    pub is_remote: bool,
    pub is_last_job: bool,
    #[serde(skip_serializing)]
    pub paused: bool,
    #[serde(skip_serializing)]
    confirmation_window: usize,
    #[serde(skip_serializing)]
    prefetched: std::collections::HashSet<i32>,
    #[serde(skip_serializing)]
    confirmations: std::collections::HashMap<i32, FileTransferSendConfirmRequest>,
    #[serde(skip_serializing)]
    file_digests: std::collections::HashMap<i32, FileDigest>,
    pub is_resume: bool,
    pub file_num: i32,
    #[serde(skip_serializing)]
    files: Vec<FileEntry>,
    pub conn_id: i32, // server only

    #[serde(skip_serializing)]
    data_stream: Option<DataStream>,
    pub total_size: u64,
    finished_size: u64,
    transferred: u64,
    enable_overwrite_detection: bool,
    file_confirmed: bool,
    // indicating the last file is skipped
    file_skipped: bool,
    file_is_waiting: bool,
    default_overwrite_strategy: Option<bool>,
    #[serde(skip_serializing)]
    digest: FileDigest,
    #[serde(skip_serializing)]
    compression: TransferCompression,
}

// Reprobe periodically so mixed-content files can regain compression.
#[derive(Debug, Default)]
struct TransferCompression {
    skip_blocks: u8,
}

impl TransferCompression {
    fn encode(&mut self, data: &[u8]) -> Option<Vec<u8>> {
        if self.skip_blocks > 0 {
            self.skip_blocks -= 1;
            return None;
        }
        let encoded = compress(data);
        if !encoded.is_empty() && encoded.len() < data.len() - data.len() / 32 {
            Some(encoded)
        } else {
            self.skip_blocks = 31;
            None
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct TransferJobMeta {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub remote: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default)]
    pub file_num: i32,
    #[serde(default)]
    pub is_remote: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct RemoveJobMeta {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub is_remote: bool,
    #[serde(default)]
    pub no_confirm: bool,
}

#[inline]
fn get_ext(name: &str) -> &str {
    if let Some(i) = name.rfind('.') {
        return &name[i + 1..];
    }
    ""
}

#[inline]
fn is_compressed_file(name: &str) -> bool {
    let compressed_exts = ["xz", "gz", "zip", "7z", "rar", "bz2", "tgz", "png", "jpg"];
    let ext = get_ext(name);
    compressed_exts.iter().any(|candidate| ext.eq_ignore_ascii_case(candidate))
}

pub fn validate_file_name_no_traversal(name: &str) -> ResultType<()> {
    if name.bytes().any(|b| b == 0) {
        bail!("file name contains null bytes");
    }
    let has_traversal = name
        .split(|c: char| c == '/' || (cfg!(windows) && c == '\\'))
        .filter(|s| !s.is_empty())
        .any(|s| s == "..");
    if has_traversal {
        bail!("path traversal detected in file name");
    }
    #[cfg(windows)]
    {
        if name.len() >= 2 {
            let bytes = name.as_bytes();
            if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                bail!("absolute path detected in file name");
            }
        }
        if name.starts_with('/') || name.starts_with('\\') {
            bail!("absolute path detected in file name");
        }
    }
    #[cfg(not(windows))]
    if name.starts_with('/') {
        bail!("absolute path detected in file name");
    }
    Ok(())
}

fn validate_transfer_file_names(files: &[FileEntry]) -> ResultType<()> {
    // Single-file transfer may use an empty relative name, because
    // the destination file path is carried by transfer metadata.
    if files.len() == 1 && files.first().map_or(false, |f| f.name.is_empty()) {
        return Ok(());
    }
    for file in files {
        if file.name.is_empty() {
            bail!("empty file name in multi-file transfer");
        }
        validate_file_name_no_traversal(&file.name)?;
    }
    Ok(())
}

#[inline]
fn validate_fs_path_argument(path: &str, arg_name: &str) -> ResultType<()> {
    if path.is_empty() {
        bail!("{arg_name} cannot be empty");
    }
    if path.bytes().any(|b| b == 0) {
        bail!("{arg_name} contains null bytes");
    }
    Ok(())
}

fn validate_no_symlink_components(base: &PathBuf, name: &str) -> ResultType<()> {
    if name.is_empty() {
        return Ok(());
    }
    let mut current = base.clone();
    for component in Path::new(name).components() {
        match component {
            std::path::Component::Normal(seg) => {
                current.push(seg);
                // Best-effort guard: path-based checks are inherently TOCTOU-prone
                // if local filesystem state changes between validation and write.
                match std::fs::symlink_metadata(&current) {
                    Ok(meta) => {
                        // This is inherent to filesystem-based checks and acknowledged as a limitation.
                        // For true protection, you'd need openat(2) / O_NOFOLLOW at write time.
                        if meta.file_type().is_symlink() {
                            bail!("symlink path component is not allowed");
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        // Component does not exist yet, continue best-effort validation.
                    }
                    Err(err) => {
                        bail!(
                            "failed to validate path component '{}': {}",
                            current.display(),
                            err
                        );
                    }
                }
            }
            std::path::Component::CurDir => {}
            _ => {
                bail!("invalid file name component");
            }
        }
    }
    Ok(())
}

/// Validate an untrusted relative file name and existing path components before joining it.
pub fn join_validated_path(base: &PathBuf, name: &str) -> ResultType<PathBuf> {
    validate_file_name_no_traversal(name)?;
    validate_no_symlink_components(base, name)?;
    Ok(TransferJob::join(base, name))
}

impl TransferJob {
    #[allow(clippy::too_many_arguments)]
    pub fn new_write(
        id: i32,
        r#type: JobType,
        remote: String,
        data_source: DataSource,
        file_num: i32,
        show_hidden: bool,
        is_remote: bool,
        enable_overwrite_detection: bool,
    ) -> Self {
        log::info!("new write {}", data_source);
        Self {
            id,
            r#type,
            remote,
            data_source,
            file_num,
            show_hidden,
            is_remote,
            files: Vec::new(),
            total_size: 0,
            enable_overwrite_detection,
            ..Default::default()
        }
    }

    pub fn with_files(mut self, files: Vec<FileEntry>) -> ResultType<Self> {
        self.set_files(files)?;
        Ok(self)
    }

    pub fn new_read(
        id: i32,
        r#type: JobType,
        remote: String,
        data_source: DataSource,
        file_num: i32,
        show_hidden: bool,
        is_remote: bool,
        enable_overwrite_detection: bool,
    ) -> ResultType<Self> {
        if r#type == JobType::Printer {
            bail!("Unsupported transfer type");
        }
        log::info!("new read {}", data_source);
        let (files, total_size) = match &data_source {
            DataSource::FilePath(p) => {
                let p = p.to_str().ok_or(anyhow!("Invalid path"))?;
                let files = get_recursive_files(p, show_hidden)?;
                let total_size = files.iter().map(|x| x.size).sum();
                (files, total_size)
            }
            DataSource::MemoryCursor(c) => (Vec::new(), c.get_ref().len() as u64),
        };
        Ok(Self {
            id,
            r#type,
            remote,
            data_source,
            file_num,
            show_hidden,
            is_remote,
            files,
            total_size,
            enable_overwrite_detection,
            ..Default::default()
        })
    }

    #[inline]
    pub fn files(&self) -> &Vec<FileEntry> {
        &self.files
    }

    #[inline]
    pub fn set_files(&mut self, files: Vec<FileEntry>) -> ResultType<()> {
        validate_transfer_file_names(&files)?;
        if let DataSource::FilePath(base) = &self.data_source {
            for file in &files {
                validate_no_symlink_components(base, &file.name)?;
            }
        }
        self.total_size = files.iter().map(|x| x.size).sum();
        self.files = files;
        Ok(())
    }

    #[inline]
    pub fn set_file_digest(&mut self, file_num: i32, size: u64, modified: u64) {
        if file_num >= self.file_num && file_num < self.file_num.saturating_add(32) {
            self.file_digests.insert(file_num, FileDigest { size, modified });
        }
    }

    pub fn set_digest(&mut self, size: u64, modified: u64) {
        self.digest.size = size;
        self.digest.modified = modified;
    }

    #[inline]
    pub fn id(&self) -> i32 {
        self.id
    }

    #[inline]
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    #[inline]
    pub fn finished_size(&self) -> u64 {
        self.finished_size
    }

    #[inline]
    pub fn transferred(&self) -> u64 {
        self.transferred
    }

    #[inline]
    pub fn file_num(&self) -> i32 {
        self.file_num
    }

    fn resolve_entry_path(&self, base: &PathBuf, name: &str) -> Option<PathBuf> {
        if self.r#type == JobType::Generic {
            match join_validated_path(base, name) {
                Ok(path) => Some(path),
                Err(err) => {
                    log::error!("Invalid file name in transfer job {}: {}", self.id, err);
                    None
                }
            }
        } else {
            Some(Self::join(base, name))
        }
    }

    pub fn modify_time(&self) {
        if self.r#type == JobType::Printer {
            return;
        }
        if let DataSource::FilePath(p) = &self.data_source {
            let file_num = self.file_num as usize;
            if file_num < self.files.len() {
                let entry = &self.files[file_num];
                let Some(path) = self.resolve_entry_path(p, &entry.name) else {
                    return;
                };
                let download_path = format!("{}.download", get_string(&path));
                let digest_path = format!("{}.digest", get_string(&path));
                std::fs::remove_file(digest_path).ok();
                std::fs::rename(download_path, &path).ok();
                filetime::set_file_mtime(
                    &path,
                    filetime::FileTime::from_unix_time(entry.modified_time as _, 0),
                )
                .ok();
            }
        }
    }

    pub fn remove_download_file(&self) {
        if self.r#type == JobType::Printer {
            return;
        }
        if let DataSource::FilePath(p) = &self.data_source {
            let file_num = self.file_num as usize;
            if file_num < self.files.len() {
                let entry = &self.files[file_num];
                let Some(path) = self.resolve_entry_path(p, &entry.name) else {
                    return;
                };
                let download_path = format!("{}.download", get_string(&path));
                let digest_path = format!("{}.digest", get_string(&path));
                std::fs::remove_file(download_path).ok();
                std::fs::remove_file(digest_path).ok();
            }
        }
    }

    #[inline]
    pub fn set_finished_size_on_resume(&mut self) {
        if self.is_resume && self.file_num > 0 {
            let finished_size: u64 = self
                .files
                .iter()
                .take(self.file_num as usize)
                .map(|file| file.size)
                .sum();
            self.finished_size = finished_size;
        }
    }

    pub async fn write(&mut self, block: FileTransferBlock) -> ResultType<()> {
        if self.r#type == JobType::Printer {
            bail!("Unsupported transfer type");
        }
        if block.id != self.id {
            bail!("Wrong id");
        }
        match &self.data_source {
            DataSource::FilePath(p) => {
                let file_num = block.file_num as usize;
                if file_num >= self.files.len() {
                    bail!("Wrong file number");
                }
                if file_num != self.file_num as usize || self.data_stream.is_none() {
                    self.modify_time();
                    if let Some(DataStream::FileStream(file)) = self.data_stream.as_mut() {
                        file.sync_all().await?;
                    }
                    self.file_num = block.file_num;
                    if let Some(digest) = self.file_digests.remove(&block.file_num) { self.digest = digest; }
                    self.file_digests.retain(|number, _| *number >= block.file_num);
                    let entry = &self.files[file_num];
                    let (path, digest_path) = {
                        let path = join_validated_path(p, &entry.name)?;
                        // NOTE: We intentionally keep path-based validation + regular file open here.
                        // This still has a known TOCTOU window for symlink races, but avoids a large
                        // cross-platform rewrite for now.
                        // Revisit with descriptor/handle-based no-follow open in future hardening.
                        if let Some(pp) = path.parent() {
                            std::fs::create_dir_all(pp).ok();
                        }
                        let file_path = get_string(&path);
                        (
                            format!("{}.download", &file_path),
                            Some(format!("{}.digest", &file_path)),
                        )
                    };
                    if let Some(dp) = digest_path.as_ref() {
                        if Path::new(dp).exists() {
                            std::fs::remove_file(dp)?;
                        }
                    }
                    self.data_stream = Some(DataStream::FileStream(File::create(&path).await?));
                    if let Some(dp) = digest_path.as_ref() {
                        std::fs::write(dp, json!(self.digest).to_string()).ok();
                    }
                }
            }
            DataSource::MemoryCursor(c) => {
                if self.data_stream.is_none() {
                    self.data_stream = Some(DataStream::BufStream(TokioBufStream::new(c.clone())));
                }
            }
        }
        if block.compressed {
            let tmp = decompress(&block.data);
            self.data_stream
                .as_mut()
                .ok_or(anyhow!("data stream is None"))?
                .write_all(&tmp)
                .await?;
            self.finished_size += tmp.len() as u64;
        } else {
            self.data_stream
                .as_mut()
                .ok_or(anyhow!("file is None"))?
                .write_all(&block.data)
                .await?;
            self.finished_size += block.data.len() as u64;
        }
        self.transferred += block.data.len() as u64;
        Ok(())
    }

    #[inline]
    pub fn join(p: &PathBuf, name: &str) -> PathBuf {
        if name.is_empty() {
            p.clone()
        } else {
            p.join(name)
        }
    }

    /// Open the data stream for the current file.
    /// Returns Ok(true) if job is done, Ok(false) otherwise.
    async fn open_data_stream(&mut self) -> ResultType<bool> {
        let file_num = self.file_num as usize;
        match &mut self.data_source {
            DataSource::FilePath(p) => {
                if file_num >= self.files.len() {
                    // job done
                    self.data_stream.take();
                    return Ok(true);
                };
                if self.data_stream.is_none() {
                    match File::open(Self::join(p, &self.files[file_num].name)).await {
                        Ok(file) => {
                            self.data_stream = Some(DataStream::FileStream(file));
                            self.file_confirmed = false;
                            self.file_is_waiting = false;
                        }
                        // On open error, behave the same as validation failure: advance
                        // to next file and return the error.
                        Err(err) => {
                            self.file_num += 1;
                            self.file_confirmed = false;
                            self.file_is_waiting = false;
                            return Err(err.into());
                        }
                    }
                }
            }
            DataSource::MemoryCursor(c) => {
                if self.data_stream.is_none() {
                    let mut t = std::io::Cursor::new(Vec::new());
                    std::mem::swap(&mut t, c);
                    self.data_stream = Some(DataStream::BufStream(TokioBufStream::new(t)));
                }
            }
        }
        Ok(false)
    }

    /// Get current file's digest (last_modified, file_size) for overwrite detection.
    async fn get_current_digest(&self) -> ResultType<(u64, u64)> {
        let meta = match self.data_stream.as_ref().ok_or(anyhow!("file is None"))? {
            DataStream::FileStream(file) => file.metadata().await?,
            DataStream::BufStream(_) => bail!("No digest for buf stream"),
        };
        let last_modified = meta
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        Ok((last_modified, meta.len()))
    }

    async fn init_data_stream(&mut self, stream: &mut hbb_common::Stream) -> ResultType<()> {
        if let Some((last_modified, file_size)) = self.init_data_stream_for_cm().await? {
            let mut response = FileResponse::new();
            response.set_digest(FileTransferDigest { id: self.id, file_num: self.file_num,
                last_modified, file_size, is_resume: self.is_resume, ..Default::default() });
            let mut message = Message::new(); message.set_file_response(response);
            stream.send(&message).await?;
        }
        for digest in self.prefetch_digests().await? {
            let mut response = FileResponse::new(); response.set_digest(digest);
            let mut message = Message::new(); message.set_file_response(response);
            stream.send(&message).await?;
        }
        Ok(())
    }

    pub async fn prefetch_digests(&mut self) -> ResultType<Vec<FileTransferDigest>> {
        let mut result = Vec::new();
        if self.paused || self.is_resume || !self.enable_overwrite_detection || self.confirmation_window == 0 { return Ok(result); }
        let current = self.file_num;
        self.prefetched.retain(|number| *number >= current);
        self.confirmations.retain(|number, _| *number >= current);
        let DataSource::FilePath(root) = &self.data_source else { return Ok(result); };
        let start = self.file_num.max(0) as usize + 1;
        for number in start..(start + self.confirmation_window).min(self.files.len()) {
            if self.prefetched.contains(&(number as i32)) { continue; }
            let meta = match tokio::fs::metadata(Self::join(root, &self.files[number].name)).await {
                Ok(meta) => meta,
                Err(_) => break, // Report source errors in normal file order.
            };
            if meta.len() > 64 * 1024 { break; }
            let modified = match meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()) {
                Some(time) => time.as_secs(),
                None => break,
            };
            self.prefetched.insert(number as i32);
            result.push(FileTransferDigest { id: self.id, file_num: number as i32,
                file_size: meta.len(), last_modified: modified, ..Default::default() });
        }
        Ok(result)
    }

    /// Initialize data stream for CM (Connection Manager) scenario.
    /// Returns digest info (last_modified, file_size) if overwrite detection is enabled,
    /// so caller can send it via IPC instead of network stream.
    /// Returns Ok(None) if job is done or already initialized.
    pub async fn init_data_stream_for_cm(&mut self) -> ResultType<Option<(u64, u64)>> {
        loop {
            if self.open_data_stream().await? { return Ok(None); }
            if let Some(confirm) = self.confirmations.remove(&self.file_num) {
                let previous = self.file_num;
                self.confirm(&confirm).await;
                if self.file_num != previous { continue; }
            }
            break;
        }
        // For overwrite detection, return digest info instead of sending via stream
        if self.r#type == JobType::Generic
            && self.enable_overwrite_detection
            && !self.file_confirmed()
            && !self.file_is_waiting()
        {
            self.set_file_is_waiting(true);
            if self.prefetched.contains(&self.file_num) { return Ok(None); }
            let digest = self.get_current_digest().await?;
            return Ok(Some(digest));
        }
        Ok(None)
    }

    pub async fn read(&mut self) -> ResultType<Option<FileTransferBlock>> {
        if self.r#type == JobType::Generic {
            if self.enable_overwrite_detection && !self.file_confirmed() {
                return Ok(None);
            }
        }

        let file_num = self.file_num as usize;
        let name = match &self.data_source {
            DataSource::FilePath(p) => {
                if file_num >= self.files.len() {
                    self.data_stream.take();
                    return Ok(None);
                };
                if self.files.len() == 1 && self.files[file_num].name.is_empty() {
                    p.file_name()
                        .map(|p| p.to_str().unwrap_or(""))
                        .unwrap_or("")
                } else {
                    &self.files[file_num].name
                }
            }
            DataSource::MemoryCursor(..) => "",
        };
        const BUF_SIZE: usize = 128 * 1024;
        // Small files should not allocate and zero a full transfer block.
        // A stale size only changes the chunk size; reads still continue to EOF.
        let buffer_size = if matches!(self.data_source, DataSource::FilePath(_)) {
            self.files[file_num].size.min(BUF_SIZE as u64).max(4096) as usize
        } else {
            BUF_SIZE
        };
        let mut buf: Vec<u8> = vec![0; buffer_size];
        let mut compressed = false;
        let mut offset: usize = 0;
        loop {
            match self
                .data_stream
                .as_mut()
                .ok_or(anyhow!("data stream is None"))?
                .read(&mut buf[offset..])
                .await
            {
                Err(err) => {
                    self.file_num += 1;
                    self.data_stream = None;
                    self.file_confirmed = false;
                    self.file_is_waiting = false;
                    return Err(err.into());
                }
                Ok(n) => {
                    offset += n;
                    if n == 0 || offset == buffer_size {
                        break;
                    }
                }
            }
        }
        unsafe { buf.set_len(offset) };
        if offset == 0 {
            if matches!(self.data_source, DataSource::MemoryCursor(_)) {
                self.data_stream.take();
                return Ok(None);
            }
            self.file_num += 1;
            self.data_stream = None;
            self.file_confirmed = false;
            self.file_is_waiting = false;
        } else {
            self.finished_size += offset as u64;
            if matches!(self.data_source, DataSource::FilePath(_)) && !is_compressed_file(name) {
                if let Some(tmp) = self.compression.encode(&buf) {
                    buf = tmp;
                    compressed = true;
                }
            }
            self.transferred += buf.len() as u64;
        }
        Ok(Some(FileTransferBlock {
            id: self.id,
            file_num: file_num as _,
            data: buf.into(),
            compressed,
            ..Default::default()
        }))
    }

    // Only for generic job and file stream
    async fn send_current_digest(&mut self, stream: &mut Stream) -> ResultType<()> {
        let (last_modified, file_size) = self.get_current_digest().await?;
        let mut msg = Message::new();
        let mut resp = FileResponse::new();
        resp.set_digest(FileTransferDigest {
            id: self.id,
            file_num: self.file_num,
            last_modified,
            file_size,
            is_resume: self.is_resume,
            ..Default::default()
        });
        msg.set_file_response(resp);
        stream.send(&msg).await?;
        log::info!(
            "id: {}, file_num: {}, digest message is sent. waiting for confirm. msg: {:?}",
            self.id,
            self.file_num,
            msg
        );
        Ok(())
    }

    pub fn set_overwrite_strategy(&mut self, overwrite_strategy: Option<bool>) {
        self.default_overwrite_strategy = overwrite_strategy;
    }

    pub fn default_overwrite_strategy(&self) -> Option<bool> {
        self.default_overwrite_strategy
    }

    pub fn set_file_confirmed(&mut self, file_confirmed: bool) {
        log::info!("id: {}, file_confirmed: {}", self.id, file_confirmed);
        self.file_confirmed = file_confirmed;
        self.file_skipped = false;
    }

    pub fn set_file_is_waiting(&mut self, file_is_waiting: bool) {
        self.file_is_waiting = file_is_waiting;
    }

    #[inline]
    pub fn file_is_waiting(&self) -> bool {
        self.file_is_waiting
    }

    #[inline]
    pub fn file_confirmed(&self) -> bool {
        self.file_confirmed
    }

    /// Indicating whether the last file is skipped
    #[inline]
    pub fn file_skipped(&self) -> bool {
        self.file_skipped
    }

    /// Indicating whether the whole task is skipped
    #[inline]
    pub fn job_skipped(&self) -> bool {
        self.file_skipped() && self.files.len() == 1
    }

    /// Check whether the job is completed after `read` returns `None`
    /// This is a helper function which gives additional lifecycle when the job reads `None`.
    /// If returns `true`, it means we can delete the job automatically. `False` otherwise.
    ///
    /// [`Note`]
    /// Conditions:
    /// 1. Files are not waiting for confirmation by peers.
    #[inline]
    pub fn job_completed(&self) -> bool {
        // has no error, Condition 2
        !self.enable_overwrite_detection || (!self.file_confirmed && !self.file_is_waiting)
    }

    /// Get job error message, useful for getting status when job had finished
    pub fn job_error(&self) -> Option<String> {
        if self.job_skipped() {
            return Some("skipped".to_string());
        }
        None
    }

    pub fn set_file_skipped(&mut self) -> bool {
        log::debug!("skip file {} in job {}", self.file_num, self.id);
        self.data_stream.take();
        self.set_file_confirmed(false);
        self.set_file_is_waiting(false);
        self.file_num += 1;
        self.file_skipped = true;
        true
    }

    async fn set_stream_offset(&mut self, file_num: usize, offset: u64) {
        if file_num >= self.files.len() {
            return;
        }
        if let DataSource::FilePath(p) = &self.data_source {
            let entry = &self.files[file_num];
            let Some(path) = self.resolve_entry_path(p, &entry.name) else {
                return;
            };
            let file_path = get_string(&path);
            let download_path = format!("{}.download", &file_path);
            let digest_path = format!("{}.digest", &file_path);

            let mut f = if Path::new(&download_path).exists() && Path::new(&digest_path).exists() {
                // If both download and digest files exist, seek (writer) to the offset
                // NOTE: same as write path: best-effort symlink validation happened earlier,
                // but this reopen remains TOCTOU-prone by design for now.
                match OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&download_path)
                    .await
                {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("Failed to open file {}: {}", download_path, e);
                        return;
                    }
                }
            } else if Path::new(&file_path).exists() {
                // If `file_path` exists, seek (reader) to the offset
                match File::open(&file_path).await {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("Failed to open file {}: {}", file_path, e);
                        return;
                    }
                }
            } else {
                log::warn!(
                    "File {} not found, cannot seek to offset {}",
                    file_path,
                    offset
                );
                return;
            };
            if f.seek(std::io::SeekFrom::Start(offset)).await.is_ok() {
                self.data_stream = Some(DataStream::FileStream(f));
                self.transferred += offset;
                self.finished_size += offset;
            }
        }
    }

    pub async fn confirm(&mut self, r: &FileTransferSendConfirmRequest) -> bool {
        if r.id != self.id || (r.file_num != self.file_num && !self.prefetched.contains(&r.file_num)) { return false; }
        self.confirmation_window = (r.confirmation_window as usize).min(16);
        if r.file_num > self.file_num && self.prefetched.contains(&r.file_num) {
            self.confirmations.insert(r.file_num, r.clone());
            return true;
        }
        if self.file_num() != r.file_num {
            // This branch will always be hit if:
            // 1. `confirm()` is called in `ui_cm_interface.rs`
            // 2. Not resuming
            //
            // It is ok. Because `confirm()` in `ui_cm_interface.rs` is only used for resuming.
            log::info!("file num truncated, ignoring");
        } else {
            match r.union {
                Some(file_transfer_send_confirm_request::Union::Skip(s)) => {
                    if s {
                        self.set_file_skipped();
                    } else {
                        self.set_file_confirmed(true);
                    }
                }
                Some(file_transfer_send_confirm_request::Union::OffsetBlk(offset)) => {
                    self.set_file_confirmed(true);
                    // If offset is greater than 0, we need to seek to the offset
                    if offset > 0 {
                        self.set_stream_offset(r.file_num as usize, offset as u64)
                            .await;
                    }
                }
                _ => {}
            }
        }
        true
    }

    #[inline]
    pub fn gen_meta(&self) -> TransferJobMeta {
        TransferJobMeta {
            id: self.id,
            remote: self.remote.to_string(),
            to: self.data_source.to_meta(),
            file_num: self.file_num,
            show_hidden: self.show_hidden,
            is_remote: self.is_remote,
        }
    }
}

#[inline]
pub fn new_error<T: std::string::ToString>(id: i32, err: T, file_num: i32) -> Message {
    let mut resp = FileResponse::new();
    resp.set_error(FileTransferError {
        id,
        error: err.to_string(),
        file_num,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn new_dir(id: i32, path: String, files: Vec<FileEntry>) -> Message {
    let mut resp = FileResponse::new();
    resp.set_dir(FileDirectory {
        id,
        path,
        entries: files,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn new_block(block: FileTransferBlock) -> Message {
    let mut resp = FileResponse::new();
    resp.set_block(block);
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn new_send_confirm(mut r: FileTransferSendConfirmRequest) -> Message {
    r.confirmation_window = 16;
    let mut msg_out = Message::new();
    let mut action = FileAction::new();
    action.set_send_confirm(r);
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_receive(
    id: i32,
    path: String,
    file_num: i32,
    files: Vec<FileEntry>,
    total_size: u64,
) -> Message {
    let mut action = FileAction::new();
    action.set_receive(FileTransferReceiveRequest {
        id,
        path,
        files,
        file_num,
        total_size,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_send(
    id: i32,
    r#type: JobType,
    path: String,
    file_num: i32,
    include_hidden: bool,
) -> Message {
    log::info!("new send: {}, id: {}", path, id);
    let mut action = FileAction::new();
    let t: file_transfer_send_request::FileType = r#type.into();
    action.set_send(FileTransferSendRequest {
        id,
        path,
        include_hidden,
        file_num,
        file_type: t.into(),
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_done(id: i32, file_num: i32) -> Message {
    let mut resp = FileResponse::new();
    resp.set_done(FileTransferDone {
        id,
        file_num,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn remove_job(id: i32, jobs: &mut Vec<TransferJob>) -> Option<TransferJob> {
    jobs.iter()
        .position(|x| x.id() == id)
        .map(|index| jobs.remove(index))
}

#[inline]
pub fn get_job(id: i32, jobs: &mut [TransferJob]) -> Option<&mut TransferJob> {
    jobs.iter_mut().find(|x| x.id() == id)
}

#[inline]
pub fn get_job_immutable(id: i32, jobs: &[TransferJob]) -> Option<&TransferJob> {
    jobs.iter().find(|x| x.id() == id)
}

async fn init_jobs(jobs: &mut Vec<TransferJob>, stream: &mut hbb_common::Stream) -> ResultType<()> {
    for job in jobs.iter_mut() {
        if job.is_last_job || job.paused {
            continue;
        }
        if let Err(err) = job.init_data_stream(stream).await {
            stream
                .send(&new_error(job.id(), err, job.file_num()))
                .await?;
        }
    }
    Ok(())
}

pub async fn handle_read_jobs(
    jobs: &mut Vec<TransferJob>,
    stream: &mut hbb_common::Stream,
) -> ResultType<String> {
    init_jobs(jobs, stream).await?;

    let mut job_log = Default::default();
    let mut finished = Vec::new();
    for job in jobs.iter_mut() {
        if job.is_last_job || job.paused {
            continue;
        }
        let started = std::time::Instant::now();
        for _ in 0..16 {
            match job.read().await {
                Err(err) => {
                    stream
                        .send(&new_error(job.id(), err, job.file_num()))
                        .await?;
                }
                Ok(Some(block)) => {
                    let file_ended = block.data.is_empty();
                    stream.send(&new_block(block)).await?;
                    // Bound each burst so control messages and cancellation get a turn.
                    if file_ended {
                        // Send the next digest immediately, but never bypass its confirmation.
                        if let Err(err) = job.init_data_stream(stream).await {
                            stream.send(&new_error(job.id(), err, job.file_num())).await?;
                            break;
                        }
                    }
                    if started.elapsed() < std::time::Duration::from_millis(2) {
                        continue;
                    }
                }
                Ok(None) => {
                    if job.job_completed() {
                        job_log = serialize_transfer_job(job, true, false, "");
                        finished.push(job.id());
                        match job.job_error() {
                            Some(err) => {
                                job_log = serialize_transfer_job(job, false, false, &err);
                                stream
                                    .send(&new_error(job.id(), err, job.file_num()))
                                    .await?
                            }
                            None => stream.send(&new_done(job.id(), job.file_num())).await?,
                        }
                    } else {
                        // waiting confirmation.
                    }
                }
            }
            break;
        }
        // Preserve sequential job ordering and overwrite confirmation.
        break;
    }
    for id in finished {
        let _ = remove_job(id, jobs);
    }
    Ok(job_log)
}

pub fn remove_all_empty_dir(path: &Path) -> ResultType<()> {
    let fd = read_dir(path, true)?;
    for entry in fd.entries.iter() {
        match entry.entry_type.enum_value() {
            Ok(FileType::Dir) => {
                remove_all_empty_dir(&path.join(&entry.name)).ok();
            }
            Ok(FileType::DirLink) | Ok(FileType::FileLink) => {
                std::fs::remove_file(path.join(&entry.name)).ok();
            }
            _ => {}
        }
    }
    std::fs::remove_dir(path).ok();
    Ok(())
}

#[inline]
pub fn remove_file(file: &str) -> ResultType<()> {
    validate_fs_path_argument(file, "file path")?;
    std::fs::remove_file(get_path(file))?;
    Ok(())
}

#[inline]
pub fn create_dir(dir: &str) -> ResultType<()> {
    validate_fs_path_argument(dir, "directory path")?;
    std::fs::create_dir_all(get_path(dir))?;
    Ok(())
}

#[inline]
pub fn rename_file(path: &str, new_name: &str) -> ResultType<()> {
    validate_fs_path_argument(path, "path")?;
    if new_name.is_empty() {
        bail!("new file name cannot be empty");
    }
    validate_file_name_no_traversal(new_name)?;
    let path = std::path::Path::new(&path);
    if path.exists() {
        let dir = path
            .parent()
            .ok_or(anyhow!("Parent directoy of {path:?} not exists"))?;
        let new_path = dir.join(&new_name);
        std::fs::rename(&path, &new_path)?;
        Ok(())
    } else {
        bail!("{path:?} not exists");
    }
}

#[inline]
pub fn transform_windows_path(entries: &mut Vec<FileEntry>) {
    for entry in entries {
        entry.name = entry.name.replace('\\', "/");
    }
}

pub enum DigestCheckResult {
    IsSame,
    NeedConfirm(FileTransferDigest),
    NoSuchFile,
}

#[inline]
pub fn is_write_need_confirmation(
    is_resume: bool,
    file_path: &str,
    digest: &FileTransferDigest,
) -> ResultType<DigestCheckResult> {
    let path = Path::new(file_path);
    let digest_file = format!("{}.digest", file_path);
    let download_file = format!("{}.download", file_path);
    if is_resume && Path::new(&digest_file).exists() && Path::new(&download_file).exists() {
        // If the digest file exists, it means the file was transferred before.
        // We can use the digest file to check whether the file is the same.
        if let Ok(content) = std::fs::read_to_string(digest_file) {
            if let Ok(local_digest) = serde_json::from_str::<FileDigest>(&content) {
                let is_identical = local_digest.modified == digest.last_modified
                    && local_digest.size == digest.file_size;
                if is_identical {
                    if let Ok(download_metadata) = std::fs::metadata(download_file) {
                        // Get the file size of the local file
                        // Only send confirmation if the file is not empty.
                        let transferred_size = download_metadata.len();
                        if transferred_size > 0 {
                            return Ok(DigestCheckResult::NeedConfirm(FileTransferDigest {
                                id: digest.id,
                                file_num: digest.file_num,
                                last_modified: digest.last_modified,
                                file_size: digest.file_size,
                                is_identical,
                                transferred_size,
                                ..Default::default()
                            }));
                        }
                    }
                }
            }
        }
    }

    if path.exists() && path.is_file() {
        let metadata = std::fs::metadata(path)?;
        let modified_time = metadata.modified()?;
        let remote_mt = Duration::from_secs(digest.last_modified);
        let local_mt = modified_time.duration_since(UNIX_EPOCH)?;
        // [Note]
        // We decide to give the decision whether to override the existing file to users,
        // which obey the behavior of the file manager in our system.
        let mut is_identical = false;
        if remote_mt == local_mt && digest.file_size == metadata.len() {
            is_identical = true;
        }
        Ok(DigestCheckResult::NeedConfirm(FileTransferDigest {
            id: digest.id,
            file_num: digest.file_num,
            last_modified: local_mt.as_secs(),
            file_size: metadata.len(),
            is_identical,
            ..Default::default()
        }))
    } else {
        // If the file does not exist, or the digest file and download file do not exist, we return NoSuchFile.
        Ok(DigestCheckResult::NoSuchFile)
    }
}

pub fn serialize_transfer_jobs(jobs: &[TransferJob]) -> String {
    let mut v = vec![];
    for job in jobs {
        let value = serde_json::to_value(job).unwrap_or_default();
        v.push(value);
    }
    serde_json::to_string(&v).unwrap_or_default()
}

pub fn serialize_transfer_job(job: &TransferJob, done: bool, cancel: bool, error: &str) -> String {
    let mut value = serde_json::to_value(job).unwrap_or_default();
    value["done"] = json!(done);
    value["cancel"] = json!(cancel);
    value["error"] = json!(error);
    serde_json::to_string(&value).unwrap_or_default()
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
