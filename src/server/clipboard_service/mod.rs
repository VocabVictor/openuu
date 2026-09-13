use super::*;
#[cfg(not(target_os = "android"))]
use crate::clipboard::clipboard_listener;
#[cfg(not(target_os = "android"))]
pub use crate::clipboard::{ClipboardContext, ClipboardSide};
pub use crate::clipboard::{CLIPBOARD_INTERVAL as INTERVAL, CLIPBOARD_NAME as NAME};
#[cfg(windows)]
use crate::ipc::{self, ClipboardFile, ClipboardNonFile, Data};
#[cfg(feature = "unix-file-copy-paste")]
pub use crate::{
    clipboard::{check_clipboard_files, FILE_CLIPBOARD_NAME as FILE_NAME},
    clipboard_file::unix_file_clip,
};
#[cfg(target_os = "android")]
use base::config::keys;
#[cfg(all(feature = "unix-file-copy-paste", target_os = "linux"))]
use clipboard::platform::unix::fuse::{init_fuse_context, uninit_fuse_context};
#[cfg(not(target_os = "android"))]
use clipboard_master::CallbackResult;
#[cfg(target_os = "android")]
use hbb_common::config::option2bool;
#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    io,
    sync::mpsc::{channel, RecvTimeoutError},
    time::Duration,
};
#[cfg(windows)]
use tokio::runtime::Runtime;

#[cfg(target_os = "android")]
static CLIPBOARD_SERVICE_OK: AtomicBool = AtomicBool::new(false);

mod run;
use run::*;
#[cfg(target_os = "linux")]
mod wayland_text;
#[cfg(target_os = "linux")]
use wayland_text::*;
mod handler;
#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests;

#[cfg(not(target_os = "android"))]
struct Handler {
    ctx: Option<ClipboardContext>,
    #[cfg(target_os = "windows")]
    stream: Option<ipc::ConnectionTmpl<parity_tokio_ipc::ConnectionClient>>,
    #[cfg(target_os = "windows")]
    rt: Option<Runtime>,
}

#[cfg(target_os = "android")]
pub fn is_clipboard_service_ok() -> bool {
    CLIPBOARD_SERVICE_OK.load(Ordering::SeqCst)
}

pub fn new(name: String) -> GenericService {
    let svc = EmptyExtraFieldService::new(name, false);
    GenericService::run(&svc.clone(), run);
    svc.sp
}
