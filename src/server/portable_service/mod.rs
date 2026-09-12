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

    pub(super) fn start_ipc_server() -> mpsc::UnboundedSender<Data> {
        let (tx, rx) = mpsc::unbounded_channel::<Data>();
        std::thread::spawn(move || start_ipc_server_async(rx));
        tx
    }

    #[tokio::main(flavor = "current_thread")]
    async fn start_ipc_server_async(rx: mpsc::UnboundedReceiver<Data>) {
        use DataPortableService::*;
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let postfix = IPC_SUFFIX;
        let quick_support = QUICK_SUPPORT.lock().unwrap().clone();

        match new_listener(postfix).await {
            Ok(mut incoming) => loop {
                {
                    tokio::select! {
                        Some(result) = incoming.next() => {
                            match result {
                                Ok(stream) => {
                                    let mut stream = Connection::new(stream);
                                    if !ipc::authorize_windows_portable_service_ipc_connection(
                                        &stream, postfix,
                                    ) {
                                        continue;
                                    }
                                    let mut consumed_token: Option<String> = None;
                                    let mut consumed_token_shmem_name: Option<String> = None;
                                    let handshake_result =
                                        ipc::portable_service_ipc_handshake_as_server(
                                            &mut stream,
                                            |token| {
                                                let (matched, matched_shmem_name) =
                                                    consume_runtime_ipc_token_if_match(token);
                                                if matched {
                                                    consumed_token = Some(token.to_owned());
                                                    consumed_token_shmem_name = matched_shmem_name;
                                                    true
                                                } else {
                                                    false
                                                }
                                            },
                                        )
                                        .await;
                                    if let Err(err) = handshake_result {
                                        if let Some(token) = consumed_token.as_deref() {
                                            restore_runtime_ipc_token_after_failed_handshake(
                                                token,
                                                consumed_token_shmem_name.as_deref(),
                                            );
                                            *STARTING.lock().unwrap() = false;
                                        }
                                        log::warn!(
                                            "Rejected portable service ipc connection due to token handshake failure: postfix={}, err={}",
                                            postfix,
                                            err
                                        );
                                        continue;
                                    }
                                    log::info!("Got portable service ipc connection");
                                    let rx_clone = rx.clone();
                                    tokio::spawn(async move {
                                        let mut stream = stream;
                                        let postfix = postfix.to_owned();
                                        let mut timer = crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
                                        let mut nack = 0;
                                        let mut rx = rx_clone.lock().await;
                                        loop {
                                            tokio::select! {
                                                res = stream.next() => {
                                                    match res {
                                                        Err(err) => {
                                                            log::info!(
                                                                "ipc{} connection closed: {}",
                                                                postfix,
                                                                err
                                                            );
                                                            break;
                                                        }
                                                        Ok(Some(Data::DataPortableService(data))) => match data {
                                                            Ping => {
                                                                stream.send(&Data::DataPortableService(Pong)).await.ok();
                                                            }
                                                            Pong => {
                                                                nack = 0;
                                                                *RUNNING.lock().unwrap() = true;
                                                                *STARTING.lock().unwrap() = false;
                                                            },
                                                            ConnCount(None) => {
                                                                if !quick_support {
                                                                    let remote_count = crate::server::AUTHED_CONNS
                                                                        .lock()
                                                                        .unwrap()
                                                                        .iter()
                                                                        .filter(|c| c.conn_type == crate::server::AuthConnType::Remote)
                                                                        .count();
                                                                    stream.send(&Data::DataPortableService(ConnCount(Some(remote_count)))).await.ok();
                                                                }
                                                            },
                                                            WillClose => {
                                                                log::info!("portable service will close");
                                                                break;
                                                            }
                                                            _=>{}
                                                        }
                                                        _=>{}
                                                    }
                                                }
                                                _ = timer.tick() => {
                                                    nack+=1;
                                                    if nack > MAX_NACK {
                                                        // In fact, this will not happen, ipc will be closed before max nack.
                                                        log::error!("max ipc nack");
                                                        break;
                                                    }
                                                    stream.send(&Data::DataPortableService(Ping)).await.ok();
                                                }
                                                Some(data) = rx.recv() => {
                                                    allow_err!(stream.send(&data).await);
                                                }
                                            }
                                        }
                                        *RUNNING.lock().unwrap() = false;
                                        *STARTING.lock().unwrap() = false;
                                    });
                                }
                                Err(err) => {
                                    log::error!("Couldn't get portable client: {:?}", err);
                                }
                            }
                        }
                    }
                }
            },
            Err(err) => {
                log::error!("Failed to start portable service ipc server: {}", err);
            }
        }
    }

    fn ipc_send(data: Data) -> ResultType<()> {
        let sender = SENDER.lock().unwrap();
        sender
            .send(data)
            .map_err(|e| anyhow!("ipc send error:{:?}", e))
    }

    fn get_cursor_info_(shmem: &mut SharedMemory, pci: PCURSORINFO) -> BOOL {
        unsafe {
            let shmem_addr_para = shmem.as_ptr().add(ADDR_CURSOR_PARA);
            if utils::counter_ready(shmem.as_ptr().add(ADDR_CURSOR_COUNTER)) {
                std::ptr::copy_nonoverlapping(shmem_addr_para, pci as _, size_of::<CURSORINFO>());
                return TRUE;
            }
            FALSE
        }
    }

    fn handle_mouse_(
        evt: &MouseEvent,
        conn: i32,
        username: String,
        argb: u32,
        simulate: bool,
        show_cursor: bool,
    ) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Mouse((
            v,
            conn,
            username,
            argb,
            simulate,
            show_cursor,
        ))))
    }

    fn handle_pointer_(evt: &PointerDeviceEvent, conn: i32) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Pointer((
            v, conn,
        ))))
    }

    fn handle_key_(evt: &KeyEvent) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Key(v)))
    }

    pub fn create_capturer(
        current_display: usize,
        display: scrap::Display,
        portable_service_running: bool,
    ) -> ResultType<Box<dyn TraitCapturer>> {
        if portable_service_running != RUNNING.lock().unwrap().clone() {
            log::info!("portable service status mismatch");
        }
        if portable_service_running && display.is_primary() {
            log::info!("Create shared memory capturer");
            return Ok(Box::new(CapturerPortable::new(current_display)));
        } else {
            log::debug!("Create capturer dxgi|gdi");
            return Ok(Box::new(
                Capturer::new(display).with_context(|| "Failed to create capturer")?,
            ));
        }
    }

    pub fn get_cursor_info(pci: PCURSORINFO) -> BOOL {
        if RUNNING.lock().unwrap().clone() {
            let mut option = SHMEM.lock().unwrap();
            option
                .as_mut()
                .map_or(FALSE, |sheme| get_cursor_info_(sheme, pci))
        } else {
            unsafe { winuser::GetCursorInfo(pci) }
        }
    }

    pub fn handle_mouse(
        evt: &MouseEvent,
        conn: i32,
        username: String,
        argb: u32,
        simulate: bool,
        show_cursor: bool,
    ) {
        if RUNNING.lock().unwrap().clone() {
            crate::input_service::update_latest_input_cursor_time(conn);
            handle_mouse_(evt, conn, username, argb, simulate, show_cursor).ok();
        } else {
            crate::input_service::handle_mouse_(evt, conn, username, argb, simulate, show_cursor);
        }
    }

    pub fn handle_pointer(evt: &PointerDeviceEvent, conn: i32) {
        if RUNNING.lock().unwrap().clone() {
            crate::input_service::update_latest_input_cursor_time(conn);
            handle_pointer_(evt, conn).ok();
        } else {
            crate::input_service::handle_pointer_(evt, conn);
        }
    }

    pub fn handle_key(evt: &KeyEvent) {
        if RUNNING.lock().unwrap().clone() {
            handle_key_(evt).ok();
        } else {
            crate::input_service::handle_key_(evt);
        }
    }

    pub fn running() -> bool {
        RUNNING.lock().unwrap().clone()
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
