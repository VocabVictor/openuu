#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::ipc::Connection;
#[cfg(not(any(target_os = "ios")))]
use crate::ipc::{self, Data};
#[cfg(target_os = "windows")]
use crate::{clipboard::ClipboardSide, ipc::ClipboardNonFile};
#[cfg(target_os = "windows")]
use base::config::keys::*;
#[cfg(not(any(target_os = "ios")))]
use base::fs::serialize_transfer_job;
use base::{
    config::keys::{OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW, OPTION_FILE_TRANSFER_MAX_FILES},
    fs::{self, get_string, is_write_need_confirmation, new_send_confirm, DigestCheckResult},
    message_proto::*,
};
#[cfg(target_os = "windows")]
use clipboard::ContextSend;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::tokio::sync::mpsc::unbounded_channel;
#[cfg(target_os = "windows")]
use hbb_common::tokio::sync::Mutex as TokioMutex;
use hbb_common::{
    allow_err, bail,
    config::{option2bool, Config},
    log,
    protobuf::Message as _,
    tokio::{
        self,
        sync::mpsc::{self, UnboundedSender},
        task::spawn_blocking,
    },
    ResultType,
};
use serde_derive::Serialize;
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
use std::iter::FromIterator;
#[cfg(not(any(target_os = "ios")))]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::Arc;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicI64, Ordering},
        RwLock,
    },
};

mod manager;
mod api;
pub use api::*;
mod ipc_runner;
mod start;
pub use start::*;
mod fs_handler;
#[cfg(windows)]
use api::cm_inner_send;
#[cfg(not(any(target_os = "ios")))]
use fs_handler::handle_fs;

/// Default maximum number of files allowed per transfer request.
/// Unit: number of files (not bytes).
#[cfg(not(any(target_os = "ios")))]
const DEFAULT_MAX_VALIDATED_FILES: usize = 10_000;

/// Maximum number of files allowed in a single file transfer request.
///
/// This limit prevents excessive I/O and memory usage when dealing with
/// large directories. It applies to:
/// - CM-side read jobs (server to client file transfers on Windows)
/// - `AllFiles` recursive directory listing operations
/// - Connection-side read jobs (non-Windows platforms)
///
/// Unit: number of files (not bytes).
/// Default: 10,000 files.
/// Configured via: `OPTION_FILE_TRANSFER_MAX_FILES` ("file-transfer-max-files")
#[cfg(not(any(target_os = "ios")))]
static MAX_VALIDATED_FILES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Get the maximum number of files allowed per transfer request.
///
/// Initializes the value from configuration (`OPTION_FILE_TRANSFER_MAX_FILES`)
/// on first call. Semantics:
/// - If the option is set to `0`, `DEFAULT_MAX_VALIDATED_FILES` (10,000) is used as a safe upper bound.
/// - If the option is unset, negative, or non-integer,
///   `usize::MAX` is used to represent "no limit" for backward compatibility with older versions
///   that did not enforce any file‑count restriction.
///   (Note: negative values are not valid for `usize` and will cause parsing to fail.)
///
/// Unit: number of files.
#[cfg(not(any(target_os = "ios")))]
#[inline]
pub fn get_max_validated_files() -> usize {
    // If `OPTION_FILE_TRANSFER_MAX_FILES` unset, negative, or non-integer, use
    // `usize::MAX` to represent "no limit", maintaining backward compatibility
    // with versions that had no file transfer restrictions.
    const NO_LIMIT_FILE_COUNT: usize = usize::MAX;
    *MAX_VALIDATED_FILES.get_or_init(|| {
        let c = crate::get_builtin_option(OPTION_FILE_TRANSFER_MAX_FILES)
            .trim()
            .parse::<usize>()
            .unwrap_or(NO_LIMIT_FILE_COUNT);
        if c == 0 {
            DEFAULT_MAX_VALIDATED_FILES
        } else {
            c
        }
    })
}

/// Check if file count exceeds the maximum allowed limit.
///
/// This check is enforced in:
/// - `start_read_job()` for CM-side read jobs
/// - `read_all_files()` for recursive directory listings
/// - `Connection::on_message()` for connection-side read jobs
///
/// # Arguments
/// * `file_count` - Number of files in the transfer request
///
/// # Returns
/// * `Ok(())` if within limit
/// * `Err(String)` with error message if limit exceeded
#[cfg(not(any(target_os = "ios")))]
pub fn check_file_count_limit(file_count: usize) -> Result<(), String> {
    let max_files = get_max_validated_files();
    if file_count > max_files {
        let msg = format!(
            "file transfer rejected: too many files ({} files exceeds limit of {}). \
             Adjust '{}' option to increase limit.",
            file_count, max_files, OPTION_FILE_TRANSFER_MAX_FILES
        );
        log::warn!("{}", msg);
        Err(msg)
    } else {
        Ok(())
    }
}

#[derive(Serialize, Clone)]
pub struct Client {
    pub id: i32,
    pub authorized: bool,
    pub disconnected: bool,
    pub is_file_transfer: bool,
    pub is_view_camera: bool,
    pub is_terminal: bool,
    pub port_forward: String,
    pub name: String,
    pub avatar: String,
    pub peer_id: String,
    pub keyboard: bool,
    pub clipboard: bool,
    pub audio: bool,
    pub file: bool,
    pub restart: bool,
    pub recording: bool,
    pub block_input: bool,
    pub privacy_mode: bool,
    pub from_switch: bool,
    pub in_voice_call: bool,
    pub incoming_voice_call: bool,
    #[serde(skip)]
    #[cfg(not(any(target_os = "ios")))]
    tx: UnboundedSender<Data>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
struct IpcTaskRunner<T: InvokeUiCM> {
    stream: Connection,
    cm: ConnectionManager<T>,
    tx: mpsc::UnboundedSender<Data>,
    rx: mpsc::UnboundedReceiver<Data>,
    close: bool,
    running: bool,
    conn_id: i32,
    #[cfg(target_os = "windows")]
    file_transfer_enabled: bool,
    #[cfg(target_os = "windows")]
    file_transfer_enabled_peer: bool,
    /// Read jobs for CM-side file reading (server to client transfers)
    read_jobs: Vec<fs::TransferJob>,
}

lazy_static::lazy_static! {
    static ref CLIENTS: RwLock<HashMap<i32, Client>> = Default::default();
}

static CLICK_TIME: AtomicI64 = AtomicI64::new(0);

#[derive(Clone)]
pub struct ConnectionManager<T: InvokeUiCM> {
    pub ui_handler: T,
}

pub trait InvokeUiCM: Send + Clone + 'static + Sized {
    fn add_connection(&self, client: &Client);

    fn remove_connection(&self, id: i32, close: bool);

    fn new_message(&self, id: i32, text: String);

    fn change_theme(&self, dark: String);

    fn change_language(&self);

    fn show_elevation(&self, show: bool);

    fn update_voice_call_state(&self, client: &Client);

    fn file_transfer_log(&self, action: &str, log: &str);
}

impl<T: InvokeUiCM> Deref for ConnectionManager<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ui_handler
    }
}

impl<T: InvokeUiCM> DerefMut for ConnectionManager<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui_handler
    }
}

/// Start a read job in CM for file transfer from server to client (Windows only).
///
/// This creates a `TransferJob` using `new_read()`, validates it, and sends the
/// initial file list back to Connection via IPC.
///
/// NOTE: This is the CM-side equivalent of `create_and_start_read_job()` in
/// `src/server/connection.rs`. On non-Windows platforms, Connection handles
/// read jobs directly. Both use `TransferJob::new_read()` with similar logic.
/// When modifying job creation or validation, ensure both paths stay in sync.
#[cfg(not(any(target_os = "ios")))]
async fn start_read_job(
    path: String,
    file_num: i32,
    include_hidden: bool,
    id: i32,
    conn_id: i32,
    overwrite_detection: bool,
    read_jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
) {
    let path_clone = path.clone();
    let result = spawn_blocking(move || -> ResultType<fs::TransferJob> {
        let data_source = fs::DataSource::FilePath(PathBuf::from(&path));
        fs::TransferJob::new_read(
            id,
            fs::JobType::Generic,
            "".to_string(),
            data_source,
            file_num,
            include_hidden,
            true,
            overwrite_detection,
        )
    })
    .await;

    match result {
        Ok(Ok(mut job)) => {
            // Optional: enforce file count limit for CM-side jobs to avoid
            // excessive I/O. This is applied on the job's file list produced
            // by `new_read`, similar to how AllFiles uses the same helper.
            if let Err(msg) = check_file_count_limit(job.files().len()) {
                if let Err(e) = tx.send(Data::ReadJobInitResult {
                    id,
                    file_num,
                    include_hidden,
                    conn_id,
                    result: Err(msg),
                }) {
                    log::error!("error sending ReadJobInitResult via IPC: {}", e);
                }
                return;
            }

            // Build FileDirectory from the job's file list and serialize
            let files = job.files().to_owned();
            let mut dir = FileDirectory::new();
            dir.id = id;
            dir.path = path_clone.clone();
            dir.entries = files.clone().into();

            let dir_bytes = match dir.write_to_bytes() {
                Ok(bytes) => bytes,
                Err(e) => {
                    if let Err(e) = tx.send(Data::ReadJobInitResult {
                        id,
                        file_num,
                        include_hidden,
                        conn_id,
                        result: Err(format!("serialize failed: {}", e)),
                    }) {
                        log::error!("error sending ReadJobInitResult via IPC: {}", e);
                    }
                    return;
                }
            };

            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Ok(dir_bytes),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }

            // Attach connection id so CM can route read blocks back correctly
            job.conn_id = conn_id;
            read_jobs.push(job);
        }
        Ok(Err(e)) => {
            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Err(format!("validation failed: {}", e)),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }
        }
        Err(e) => {
            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Err(format!("validation task failed: {}", e)),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }
        }
    }
}

/// Process read jobs periodically, reading file blocks and sending them via IPC.
///
/// NOTE: This is the CM-side equivalent of `handle_read_jobs()` in
/// `libs/base/src/fs.rs`. The logic mirrors that implementation
/// but communicates via IPC instead of direct network stream.
/// When modifying job processing logic, ensure both implementations stay in sync.
#[cfg(not(any(target_os = "ios")))]
async fn handle_read_jobs_tick(
    jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
    conn_id: i32,
) -> ResultType<()> {
    let mut finished = Vec::new();

    for job in jobs.iter_mut() {
        if job.is_last_job || job.paused {
            continue;
        }

        // Initialize data stream if needed (opens file, sends digest for overwrite detection)
        if let Err(err) = init_read_job_for_cm(job, tx, conn_id).await {
            if let Err(e) = tx.send(Data::FileReadError {
                id: job.id,
                file_num: job.file_num(),
                err: format!("{}", err),
                conn_id,
            }) {
                log::error!("error sending FileReadError via IPC: {}", e);
            }
            finished.push(job.id);
            continue;
        }

        // Bound bursts just like the direct sender, including small-file EOF blocks.
        let started = std::time::Instant::now();
        for _ in 0..16 {
        match job.read().await {
            Err(err) => {
                if let Err(e) = tx.send(Data::FileReadError {
                    id: job.id,
                    file_num: job.file_num(),
                    err: format!("{}", err),
                    conn_id,
                }) {
                    log::error!("error sending FileReadError via IPC: {}", e);
                }
                // Mark job as finished to prevent infinite retries.
                // Connection side will have already removed cm_read_job_ids
                // after receiving FileReadError, so continuing would be pointless.
                finished.push(job.id);
            }
            Ok(Some(block)) => {
                let file_ended = block.data.is_empty();
                if let Err(e) = tx.send(Data::FileBlockFromCM {
                    id: block.id,
                    file_num: block.file_num,
                    data: block.data,
                    compressed: block.compressed,
                    conn_id,
                }) {
                    log::error!("error sending FileBlockFromCM via IPC: {}", e);
                    break;
                }
                if file_ended {
                    if let Err(err) = init_read_job_for_cm(job, tx, conn_id).await {
                        tx.send(Data::FileReadError { id: job.id, file_num: job.file_num(),
                            err: err.to_string(), conn_id })?;
                        finished.push(job.id);
                        break;
                    }
                }
                if started.elapsed() < std::time::Duration::from_millis(2) {
                    continue;
                }
            }
            Ok(None) => {
                if job.job_completed() {
                    finished.push(job.id);
                    match job.job_error() {
                        Some(err) => {
                            if let Err(e) = tx.send(Data::FileReadError {
                                id: job.id,
                                file_num: job.file_num(),
                                err,
                                conn_id,
                            }) {
                                log::error!("error sending FileReadError via IPC: {}", e);
                            }
                        }
                        None => {
                            if let Err(e) = tx.send(Data::FileReadDone {
                                id: job.id,
                                file_num: job.file_num(),
                                conn_id,
                            }) {
                                log::error!("error sending FileReadDone via IPC: {}", e);
                            }
                        }
                    }
                }
                // else: waiting for confirmation from peer
            }
        }
        break;
        }
        // Break to handle jobs one by one.
        break;
    }

    for id in finished {
        let _ = fs::remove_job(id, jobs);
    }

    Ok(())
}

/// Initialize a read job's data stream and handle digest sending for overwrite detection.
///
/// NOTE: This is the CM-side equivalent of `TransferJob::init_data_stream()` in
/// `libs/base/src/fs.rs`. It calls `init_data_stream_for_cm()` and sends
/// digest via IPC instead of direct network stream.
/// When modifying initialization or digest logic, ensure both paths stay in sync.
#[cfg(not(any(target_os = "ios")))]
async fn init_read_job_for_cm(
    job: &mut fs::TransferJob,
    tx: &UnboundedSender<Data>,
    conn_id: i32,
) -> ResultType<()> {
    // Initialize data stream and get digest info if overwrite detection is needed
    match job.init_data_stream_for_cm().await? {
        Some((last_modified, file_size)) => {
            // Send digest via IPC for overwrite detection
            if let Err(e) = tx.send(Data::FileDigestFromCM {
                id: job.id,
                file_num: job.file_num(),
                last_modified,
                file_size,
                is_resume: job.is_resume,
                conn_id,
            }) {
                log::error!("error sending FileDigestFromCM via IPC: {}", e);
            }
        }
        None => {
            // Job done or already initialized, nothing to do
        }
    }
    for digest in job.prefetch_digests().await? {
        tx.send(Data::FileDigestFromCM { id: digest.id, file_num: digest.file_num,
            last_modified: digest.last_modified, file_size: digest.file_size,
            is_resume: digest.is_resume, conn_id })?;
    }

    Ok(())
}

#[cfg(not(any(target_os = "ios")))]
async fn read_all_files(
    path: String,
    include_hidden: bool,
    id: i32,
    conn_id: i32,
    tx: &UnboundedSender<Data>,
) {
    let path_clone = path.clone();
    let result = spawn_blocking(move || fs::get_recursive_files(&path, include_hidden)).await;

    let result = match result {
        Ok(Ok(files)) => {
            // Check file count limit to prevent excessive I/O and resource usage
            if let Err(msg) = check_file_count_limit(files.len()) {
                Err(msg)
            } else {
                // Serialize FileDirectory to protobuf bytes
                let mut fd = FileDirectory::new();
                fd.id = id;
                fd.path = path_clone.clone();
                fd.entries = files.into();
                match fd.write_to_bytes() {
                    Ok(bytes) => Ok(bytes),
                    Err(e) => Err(format!("serialize failed: {}", e)),
                }
            }
        }
        Ok(Err(e)) => Err(format!("{}", e)),
        Err(e) => Err(format!("task failed: {}", e)),
    };

    if let Err(e) = tx.send(Data::AllFilesResult {
        id,
        conn_id,
        path: path_clone,
        result,
    }) {
        log::error!("error sending AllFilesResult via IPC: {}", e);
    }
}

#[cfg(not(any(target_os = "ios")))]
async fn read_empty_dirs(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
    let path = dir.to_owned();
    let path_clone = dir.to_owned();

    if let Ok(Ok(fds)) =
        spawn_blocking(move || fs::get_empty_dirs_recursive(&path, include_hidden)).await
    {
        let mut msg_out = Message::new();
        let mut file_response = FileResponse::new();
        file_response.set_empty_dirs(ReadEmptyDirsResponse {
            path: path_clone,
            empty_dirs: fds,
            ..Default::default()
        });
        msg_out.set_file_response(file_response);
        send_raw(msg_out, tx);
    }
}

#[cfg(not(any(target_os = "ios")))]
async fn read_dir(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
    let path = {
        if dir.is_empty() {
            Config::get_home()
        } else {
            fs::get_path(dir)
        }
    };
    let result = spawn_blocking(move || fs::read_dir(&path, include_hidden)).await;
    let msg_out = match result {
        Ok(Ok(fd)) => {
            let mut msg_out = Message::new();
            let mut file_response = FileResponse::new();
            file_response.set_dir(fd);
            msg_out.set_file_response(file_response);
            msg_out
        }
        Ok(Err(err)) => fs::new_error(0, err, -1),
        Err(err) => fs::new_error(0, err, -1),
    };
    send_raw(msg_out, tx);
}

#[cfg(not(any(target_os = "ios")))]
async fn handle_result<F: std::fmt::Display, S: std::fmt::Display>(
    res: std::result::Result<std::result::Result<(), F>, S>,
    id: i32,
    file_num: i32,
    tx: &UnboundedSender<Data>,
) {
    match res {
        Err(err) => {
            send_raw(fs::new_error(id, err, file_num), tx);
        }
        Ok(Err(err)) => {
            send_raw(fs::new_error(id, err, file_num), tx);
        }
        Ok(Ok(())) => {
            send_raw(fs::new_done(id, file_num), tx);
        }
    }
}

#[cfg(not(any(target_os = "ios")))]
async fn remove_file(path: String, id: i32, file_num: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || fs::remove_file(&path)).await,
        id,
        file_num,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn create_dir(path: String, id: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || fs::create_dir(&path)).await,
        id,
        0,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn rename_file(path: String, new_name: String, id: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || fs::rename_file(&path, &new_name)).await,
        id,
        0,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn remove_dir(path: String, id: i32, recursive: bool, tx: &UnboundedSender<Data>) {
    let path = fs::get_path(&path);
    handle_result(
        spawn_blocking(move || {
            if recursive {
                fs::remove_all_empty_dir(&path)
            } else {
                std::fs::remove_dir(&path).map_err(|err| err.into())
            }
        })
        .await,
        id,
        0,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
fn send_raw(msg: Message, tx: &UnboundedSender<Data>) {
    match msg.write_to_bytes() {
        Ok(bytes) => {
            allow_err!(tx.send(Data::RawMessage(bytes)));
        }
        err => allow_err!(err),
    }
}

#[cfg(test)]
mod tests {
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
}
