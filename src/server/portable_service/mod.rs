use crate::{
    ipc::{self, new_listener, Connection, Data, DataPortableService, IPC_TOKEN_LEN},
    platform::{
        set_path_permission, set_path_permission_for_portable_service_shmem_dir,
        set_path_permission_for_portable_service_shmem_file,
        validate_path_for_portable_service_shmem_dir,
    },
};
use base::message_proto::{KeyEvent, MouseEvent};
use core::slice;
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail, libc, log,
    protobuf::Message,
    tokio::{self, sync::mpsc},
    ResultType,
};
#[cfg(feature = "vram")]
use scrap::AdapterDevice;
use scrap::{Capturer, Frame, TraitCapturer, TraitPixelBuffer};
use shared_memory::*;
use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use winapi::{
    shared::minwindef::{BOOL, FALSE, TRUE},
    um::winuser::{self, CURSORINFO, PCURSORINFO},
};
use windows::Win32::Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ};

use super::video_qos;

const SIZE_COUNTER: usize = size_of::<i32>() * 2;
const FRAME_ALIGN: usize = 64;

const ADDR_IPC_TOKEN: usize = 0;
const ADDR_CURSOR_PARA: usize = ADDR_IPC_TOKEN + IPC_TOKEN_LEN;
const ADDR_CURSOR_COUNTER: usize = ADDR_CURSOR_PARA + size_of::<CURSORINFO>();

const ADDR_CAPTURER_PARA: usize = ADDR_CURSOR_COUNTER + SIZE_COUNTER;
const ADDR_CAPTURE_FRAME_INFO: usize = ADDR_CAPTURER_PARA + size_of::<CapturerPara>();
const ADDR_CAPTURE_WOULDBLOCK: usize = ADDR_CAPTURE_FRAME_INFO + size_of::<FrameInfo>();
const ADDR_CAPTURE_FRAME_COUNTER: usize = ADDR_CAPTURE_WOULDBLOCK + size_of::<i32>();

const ADDR_CAPTURE_FRAME: usize =
    (ADDR_CAPTURE_FRAME_COUNTER + SIZE_COUNTER + FRAME_ALIGN - 1) / FRAME_ALIGN * FRAME_ALIGN;
const MIN_RUNTIME_SHMEM_LEN: usize = ADDR_CAPTURE_FRAME + FRAME_ALIGN;

const IPC_SUFFIX: &str = "_portable_service";
pub const SHMEM_NAME: &str = "_portable_service";
pub const SHMEM_ARG_PREFIX: &str = "--portable-service-shmem-name=";
const SHMEM_PARENT_DIR: &str = "portable_service_shmem";
const SHMEM_NAME_MAX_LEN: usize = 64;
const MAX_NACK: usize = 3;
const PORTABLE_SERVICE_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_DXGI_FAIL_TIME: usize = 5;

mod shmem_helpers;
pub use shmem_helpers::*;
mod shmem;
pub use shmem::*;
mod utils;
#[cfg(test)]
mod tests;

// functions called in separate SYSTEM user process.
pub mod server {
    use base::message_proto::PointerDeviceEvent;

    use crate::display_service;

    use super::*;

    mod run;
    pub use run::*;
    mod capture;
    use capture::*;
    mod ipc_client;
    use ipc_client::*;

    lazy_static::lazy_static! {
        static ref EXIT: Arc<Mutex<bool>> = Default::default();
        static ref FORCE_EXIT_ARMED: AtomicBool = AtomicBool::new(false);
    }

}

// functions called in main process.
pub mod client {
    use super::*;
    use crate::display_service;
    use base::message_proto::PointerDeviceEvent;
    use hbb_common::anyhow::Context;
    use scrap::PixelBuffer;

    mod runtime_state;
    use runtime_state::*;
    mod start;
    pub use start::*;
    mod capturer;
    pub use capturer::*;
    mod ipc_server;
    use ipc_server::*;
    mod api;
    pub use api::*;

    lazy_static::lazy_static! {
        static ref RUNNING: Arc<Mutex<bool>> = Default::default();
        static ref STARTING: Arc<Mutex<bool>> = Default::default();
        static ref STARTING_TOKEN: AtomicU64 = AtomicU64::new(0);
        static ref SHMEM: Arc<Mutex<Option<SharedMemory>>> = Default::default();
        static ref SHMEM_RUNTIME_NAME: Arc<Mutex<Option<String>>> = Default::default();
        static ref IPC_RUNTIME_TOKEN: Arc<Mutex<Option<String>>> = Default::default();
        static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(client::start_ipc_server());
        static ref QUICK_SUPPORT: Arc<Mutex<bool>> = Default::default();
    }

}

#[repr(C)]
pub struct CapturerPara {
    recreate: bool,
    current_display: usize,
    timeout_ms: i32,
}

#[repr(C)]
pub struct FrameInfo {
    length: usize,
    width: usize,
    height: usize,
}
