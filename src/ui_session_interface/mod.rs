use crate::{
    common::{get_supported_keyboard_modes, is_keyboard_mode_supported},
    input::{
        MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT, MOUSE_TYPE_DOWN, MOUSE_TYPE_MASK,
        MOUSE_TYPE_TRACKPAD, MOUSE_TYPE_UP, MOUSE_TYPE_WHEEL,
    },
    ui_interface::use_texture_render,
};
use async_trait::async_trait;
#[cfg(all(target_os = "windows", not(feature = "flutter")))]
use base::config::keys;
#[cfg(not(feature = "flutter"))]
use base::fs;
use base::message_proto::*;
use bytes::Bytes;
use hbb_common::{
    allow_err,
    config::{Config, LocalConfig, PeerConfig},
    get_version_number, log,
    rendezvous_proto::ConnType,
    tokio::{
        self,
        sync::mpsc,
        time::{Duration as TokioDuration, Instant},
    },
    whoami, Stream,
};
use rdev::{Event, EventType::*, KeyCode};
#[cfg(all(feature = "vram", feature = "flutter"))]
use std::ffi::c_void;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
    time::SystemTime,
};
use uuid::Uuid;

use crate::client::io_loop::Remote;
use crate::client::{
    check_if_retry, handle_hash, handle_login_error, handle_login_from_ui, handle_test_delay,
    input_os_password, send_mouse, send_pointer_device_event, FileManager, Key, LoginConfigHandler,
    QualityStatus, KEY_MAP,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::common::GrabState;
use crate::keyboard;
use crate::{client::Data, client::Interface};

mod options;
mod video;
mod misc;
mod key_events;
mod key_simulation;
mod messaging;
mod mouse;
mod connection;
mod invoke_ui;
pub use invoke_ui::*;
mod interface;
mod io_loop;
pub use io_loop::*;
use io_loop::send_note;

const CHANGE_RESOLUTION_VALID_TIMEOUT_SECS: u64 = 15;

#[derive(Clone, Default)]
pub struct Session<T: InvokeUiSession> {
    pub password: String,
    pub args: Vec<String>,
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    pub thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub ui_handler: T,
    pub server_keyboard_enabled: Arc<RwLock<bool>>,
    pub server_file_transfer_enabled: Arc<RwLock<bool>>,
    pub server_clipboard_enabled: Arc<RwLock<bool>>,
    pub last_change_display: Arc<Mutex<ChangeDisplayRecord>>,
    pub connection_round_state: Arc<Mutex<ConnectionRoundState>>,
    // Indicate whether the session is reconnected.
    // Used to auto start file transfer after reconnection.
    pub reconnect_count: Arc<AtomicUsize>,
    pub last_audit_note: Arc<Mutex<String>>,
    pub audit_guid: Arc<Mutex<String>>,
}

#[derive(Clone)]
pub struct SessionPermissionConfig {
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub server_keyboard_enabled: Arc<RwLock<bool>>,
    pub server_file_transfer_enabled: Arc<RwLock<bool>>,
    pub server_clipboard_enabled: Arc<RwLock<bool>>,
}

pub struct ChangeDisplayRecord {
    time: Instant,
    display: i32,
    width: i32,
    height: i32,
}

enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

/// ConnectionRoundState is used to control the reconnecting logic.
pub struct ConnectionRoundState {
    round: u32,
    state: ConnectionState,
}

impl ConnectionRoundState {
    pub fn new_round(&mut self) -> u32 {
        self.round += 1;
        self.state = ConnectionState::Connecting;
        self.round
    }

    pub fn set_connected(&mut self) {
        self.state = ConnectionState::Connected;
    }

    pub fn is_round_gt(&self, round: u32) -> bool {
        if round == u32::MAX && self.round == 0 {
            true
        } else {
            round < self.round
        }
    }

    pub fn set_disconnected(&mut self, round: u32) -> bool {
        if self.is_round_gt(round) {
            false
        } else {
            self.state = ConnectionState::Disconnected;
            true
        }
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Connected)
    }
}

impl Default for ConnectionRoundState {
    fn default() -> Self {
        Self {
            round: 0,
            state: ConnectionState::Connecting,
        }
    }
}

impl Default for ChangeDisplayRecord {
    fn default() -> Self {
        Self {
            time: Instant::now()
                - TokioDuration::from_secs(CHANGE_RESOLUTION_VALID_TIMEOUT_SECS + 1),
            display: 0,
            width: 0,
            height: 0,
        }
    }
}

impl ChangeDisplayRecord {
    fn new(display: i32, width: i32, height: i32) -> Self {
        Self {
            time: Instant::now(),
            display,
            width,
            height,
        }
    }

    pub fn is_the_same_record(&self, display: i32, width: i32, height: i32) -> bool {
        self.time.elapsed().as_secs() < CHANGE_RESOLUTION_VALID_TIMEOUT_SECS
            && self.display == display
            && self.width == width
            && self.height == height
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl SessionPermissionConfig {
    pub fn is_text_clipboard_required(&self) -> bool {
        *self.server_clipboard_enabled.read().unwrap()
            && *self.server_keyboard_enabled.read().unwrap()
            && !self.lc.read().unwrap().disable_clipboard.v
            && !self.lc.read().unwrap().get_toggle_option("view-only")
    }

    #[cfg(feature = "unix-file-copy-paste")]
    pub fn is_file_clipboard_required(&self) -> bool {
        let lc = self.lc.read().unwrap();
        *self.server_keyboard_enabled.read().unwrap()
            && *self.server_file_transfer_enabled.read().unwrap()
            && lc.enable_file_copy_paste.v
            && !lc.get_toggle_option("view-only")
    }
}


