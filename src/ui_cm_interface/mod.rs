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
    allow_err,
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
mod read_jobs;
mod fs_ops;
#[cfg(test)]
mod tests;
#[cfg(windows)]
use api::cm_inner_send;
#[cfg(not(any(target_os = "ios")))]
use fs_handler::handle_fs;
#[cfg(not(any(target_os = "ios")))]
use read_jobs::{handle_read_jobs_tick, start_read_job};
#[cfg(not(any(target_os = "ios")))]
use fs_ops::{create_dir, read_all_files, read_dir, read_empty_dirs, remove_dir, remove_file, rename_file, send_raw};

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


