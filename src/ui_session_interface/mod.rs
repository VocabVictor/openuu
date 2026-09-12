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

impl<T: InvokeUiSession> Session<T> {

    pub fn send_chat(&self, text: String) {
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    // Terminal methods
    pub fn open_terminal(&self, terminal_id: i32, rows: u32, cols: u32) {
        let mut action = TerminalAction::new();
        action.set_open(OpenTerminal {
            terminal_id,
            rows,
            cols,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn send_terminal_input(&self, terminal_id: i32, data: String) {
        let mut action = TerminalAction::new();
        action.set_data(TerminalData {
            terminal_id,
            data: bytes::Bytes::from(data.into_bytes()),
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn resize_terminal(&self, terminal_id: i32, rows: u32, cols: u32) {
        let mut action = TerminalAction::new();
        action.set_resize(ResizeTerminal {
            terminal_id,
            rows,
            cols,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn close_terminal(&self, terminal_id: i32) {
        let mut action = TerminalAction::new();
        action.set_close(CloseTerminal {
            terminal_id,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn send_touch_scale(&self, scale: i32, alt: bool, ctrl: bool, shift: bool, command: bool) {
        let scale_evt = TouchScaleUpdate {
            scale,
            ..Default::default()
        };
        let mut touch_evt = TouchEvent::new();
        touch_evt.set_scale_update(scale_evt);
        let mut evt = PointerDeviceEvent::new();
        evt.set_touch_event(touch_evt);
        send_pointer_device_event(evt, alt, ctrl, shift, command, self);
    }

    pub fn send_touch_pan_event(
        &self,
        event: &str,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let mut touch_evt = TouchEvent::new();
        match event {
            "pan_start" => {
                touch_evt.set_pan_start(TouchPanStart {
                    x,
                    y,
                    ..Default::default()
                });
            }
            "pan_update" => {
                let (x, y) = self.get_scroll_xy((x, y));
                touch_evt.set_pan_update(TouchPanUpdate {
                    x,
                    y,
                    ..Default::default()
                });
            }
            "pan_end" => {
                touch_evt.set_pan_end(TouchPanEnd {
                    x,
                    y,
                    ..Default::default()
                });
            }
            _ => {
                log::warn!("unknown touch pan event: {}", event);
                return;
            }
        };
        let mut evt = PointerDeviceEvent::new();
        evt.set_touch_event(touch_evt);
        send_pointer_device_event(evt, alt, ctrl, shift, command, self);
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn is_scroll_reverse_mode(&self) -> bool {
        self.lc.read().unwrap().reverse_mouse_wheel.eq("Y")
    }

    #[inline]
    fn get_scroll_xy(&self, xy: (i32, i32)) -> (i32, i32) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.is_scroll_reverse_mode() {
            return (-xy.0, -xy.1);
        }
        xy
    }

    pub fn send_mouse(
        &self,
        mut mask: i32,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        #[allow(unused_mut)]
        let mut command = command;
        #[cfg(windows)]
        {
            if !command && crate::platform::windows::get_win_key_state() {
                command = true;
            }
        }

        // Compute event type once using MOUSE_TYPE_MASK for reuse
        let event_type = mask & MOUSE_TYPE_MASK;
        let (x, y) = if event_type == MOUSE_TYPE_WHEEL || event_type == MOUSE_TYPE_TRACKPAD {
            self.get_scroll_xy((x, y))
        } else {
            (x, y)
        };

        // #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let (alt, ctrl, shift, command) =
            keyboard::client::get_modifiers_state(alt, ctrl, shift, command);
        let is_left = (mask & (MOUSE_BUTTON_LEFT << 3)) > 0;
        let is_right = (mask & (MOUSE_BUTTON_RIGHT << 3)) > 0;
        if is_left ^ is_right {
            let swap_lr = self.get_toggle_option("swap-left-right-mouse".to_string());
            if swap_lr {
                if is_left {
                    mask = (mask & (!(MOUSE_BUTTON_LEFT << 3))) | (MOUSE_BUTTON_RIGHT << 3);
                } else {
                    mask = (mask & (!(MOUSE_BUTTON_RIGHT << 3))) | (MOUSE_BUTTON_LEFT << 3);
                }
            }
        }

        send_mouse(mask, x, y, alt, ctrl, shift, command, self);
        // on macos, ctrl + left button down = right button down, up won't emit, so we need to
        // emit up myself if peer is not macos
        // to-do: how about ctrl + left from win to macos
        if cfg!(target_os = "macos") {
            let buttons = mask >> 3;
            if buttons == MOUSE_BUTTON_LEFT
                && event_type == MOUSE_TYPE_DOWN
                && ctrl
                && self.peer_platform() != "Mac OS"
            {
                self.send_mouse(
                    (MOUSE_BUTTON_LEFT << 3 | MOUSE_TYPE_UP) as _,
                    x,
                    y,
                    alt,
                    ctrl,
                    shift,
                    command,
                );
            }
        }
    }

    pub fn reconnect(&self, force_relay: bool) {
        // 1. If current session is connecting, do not reconnect.
        // 2. If the connection is established, send `Data::Close`.
        // 3. If the connection is disconnected, do nothing.
        let mut connection_round_state_lock = self.connection_round_state.lock().unwrap();
        if self.thread.lock().unwrap().is_some() {
            match connection_round_state_lock.state {
                ConnectionState::Connecting => return,
                ConnectionState::Connected => self.send(Data::Close),
                ConnectionState::Disconnected => {}
            }
        }
        let round = connection_round_state_lock.new_round();
        drop(connection_round_state_lock);

        let cloned = self.clone();

        // override only if true
        if true == force_relay {
            let mut lc = self.lc.write().unwrap();
            lc.force_relay = true;
            // An explicit retry-via-relay is a decision about this peer, not transport
            // necessity: Relay-only ICE for this round like any force-always-relay session,
            // and it is the one kind of relay that belongs in the peer's saved config.
            lc.policy_relay = true;
            lc.peer_relay = true;
        }
        self.lc.write().unwrap().peer_info = None;
        self.reconnect_count.fetch_add(1, Ordering::SeqCst);
        let mut lock = self.thread.lock().unwrap();
        // No need to join the previous thread, because it will exit automatically.
        // And the previous thread will not change important states.
        *lock = Some(std::thread::spawn(move || {
            io_loop(cloned, round);
        }));
    }

    #[cfg(not(feature = "flutter"))]
    pub fn get_icon_path(&self, file_type: i32, ext: String) -> String {
        let mut path = Config::icon_path();
        if file_type == FileType::DirLink as i32 {
            let new_path = path.join("dir_link");
            if !std::fs::metadata(&new_path).is_ok() {
                #[cfg(windows)]
                allow_err!(std::os::windows::fs::symlink_file(&path, &new_path));
                #[cfg(not(windows))]
                allow_err!(std::os::unix::fs::symlink(&path, &new_path));
            }
            path = new_path;
        } else if file_type == FileType::File as i32 {
            if !ext.is_empty() {
                path = path.join(format!("file.{}", ext));
            } else {
                path = path.join("file");
            }
            if !std::fs::metadata(&path).is_ok() {
                allow_err!(std::fs::File::create(&path));
            }
        } else if file_type == FileType::FileLink as i32 {
            let new_path = path.join("file_link");
            if !std::fs::metadata(&new_path).is_ok() {
                path = path.join("file");
                if !std::fs::metadata(&path).is_ok() {
                    allow_err!(std::fs::File::create(&path));
                }
                #[cfg(windows)]
                allow_err!(std::os::windows::fs::symlink_file(&path, &new_path));
                #[cfg(not(windows))]
                allow_err!(std::os::unix::fs::symlink(&path, &new_path));
            }
            path = new_path;
        } else if file_type == FileType::DirDrive as i32 {
            if cfg!(windows) {
                path = fs::get_path("C:");
            } else if cfg!(target_os = "macos") {
                if let Ok(entries) = fs::get_path("/Volumes/").read_dir() {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            path = entry.path();
                            break;
                        }
                    }
                }
            }
        }
        fs::get_string(&path)
    }

    pub fn login(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
    ) {
        self.send(Data::Login((os_username, os_password, password, remember)));
    }

    pub fn send2fa(&self, code: String, trust_this_device: bool) {
        let mut msg_out = Message::new();
        let hwid = if trust_this_device {
            crate::get_hwid()
        } else {
            Bytes::new()
        };
        self.lc.write().unwrap().set_option(
            "trust-this-device".to_string(),
            if trust_this_device { "Y" } else { "" }.to_string(),
        );
        msg_out.set_auth_2fa(Auth2FA {
            code,
            hwid,
            ..Default::default()
        });
        self.send(Data::Message(msg_out));
    }

    pub fn get_enable_trusted_devices(&self) -> bool {
        self.lc.read().unwrap().enable_trusted_devices
    }

    pub fn new_rdp(&self) {
        self.send(Data::NewRDP);
    }

    pub fn close(&self) {
        self.send(Data::Close);
    }

    pub fn continue_insecure_connection(&self, continue_insecure: bool) {
        let data = if continue_insecure {
            Data::ContinueInsecureConnection
        } else {
            Data::RejectInsecureConnection
        };
        self.send(data);
    }

    fn try_auto_start_job_str(is_reconnected: bool, job_str: &str) -> Option<String> {
        if is_reconnected {
            let job_str = job_str.trim();
            if let Some(stripped) = job_str.strip_suffix('}') {
                format!(r#"{},"auto_start": true}}"#, stripped).into()
            } else {
                // unreachable in normal cases
                log::warn!(
                    "The last character is not '}}': {}, auto start is ignored on flutter",
                    job_str
                );
                Some(job_str.to_owned())
            }
        } else {
            None
        }
    }

    pub fn load_last_jobs(&self) {
        self.clear_all_jobs();
        let pc = self.load_config();
        if pc.transfer.write_jobs.is_empty() && pc.transfer.read_jobs.is_empty() {
            // no last jobs
            return;
        }
        let reconnect_count_thr = if cfg!(feature = "flutter") { 0 } else { 1 };
        let is_reconnected = self.reconnect_count.load(Ordering::SeqCst) > reconnect_count_thr;
        // TODO: can add a confirm dialog
        let mut cnt = 1;
        for job_str in pc.transfer.read_jobs.iter() {
            if !job_str.is_empty() {
                self.load_last_job(
                    cnt,
                    Self::try_auto_start_job_str(is_reconnected, job_str)
                        .as_deref()
                        .unwrap_or(job_str),
                    is_reconnected,
                );
                cnt += 1;
                log::info!("restore read_job: {:?}", job_str);
            }
        }
        for job_str in pc.transfer.write_jobs.iter() {
            if !job_str.is_empty() {
                self.load_last_job(
                    cnt,
                    Self::try_auto_start_job_str(is_reconnected, job_str)
                        .as_deref()
                        .unwrap_or(job_str),
                    is_reconnected,
                );
                cnt += 1;
                log::info!("restore write_job: {:?}", job_str);
            }
        }
        self.update_transfer_list();
    }

    pub fn elevate_direct(&self) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        self.send(Data::ElevateDirect);
    }

    pub fn elevate_with_logon(&self, username: String, password: String) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        self.send(Data::ElevateWithLogon(username, password));
    }

    #[cfg(any(target_os = "android", target_os = "ios", not(feature = "flutter")))]
    pub fn switch_sides(&self) {}

    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[tokio::main(flavor = "current_thread")]
    pub async fn switch_sides(&self) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                if conn
                    .send(&crate::ipc::Data::SwitchSidesRequest(self.get_id()))
                    .await
                    .is_ok()
                {
                    if let Ok(Some(data)) = conn.next_timeout(1000).await {
                        match data {
                            crate::ipc::Data::SwitchSidesRequest(str_uuid) => {
                                if let Ok(uuid) = Uuid::from_str(&str_uuid) {
                                    let mut misc = Misc::new();
                                    misc.set_switch_sides_request(SwitchSidesRequest {
                                        uuid: Bytes::from(uuid.as_bytes().to_vec()),
                                        ..Default::default()
                                    });
                                    let mut msg_out = Message::new();
                                    msg_out.set_misc(misc);
                                    self.send(Data::Message(msg_out));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(err) => {
                log::info!("server not started (will try to start): {}", err);
            }
        }
    }

    #[inline]
    pub fn request_voice_call(&self) {
        #[cfg(target_os = "linux")]
        std::thread::spawn(crate::ipc::start_pa);
        self.send(Data::NewVoiceCall);
    }

    #[inline]
    pub fn close_voice_call(&self) {
        self.send(Data::CloseVoiceCall);
    }

    pub fn send_selected_session_id(&self, sid: String) {
        if let Ok(sid) = sid.parse::<u32>() {
            self.lc.write().unwrap().selected_windows_session_id = Some(sid);
            let mut misc = Misc::new();
            misc.set_selected_sid(sid);
            let mut msg = Message::new();
            msg.set_misc(misc);
            self.send(Data::Message(msg));
            let pi = self.lc.read().unwrap().peer_info.clone();
            if let Some(pi) = pi {
                if pi.windows_sessions.current_sid == sid {
                    if self.is_file_transfer() {
                        if pi.username.is_empty() {
                            self.on_error(
                                "No active console user logged on, please connect and logon first.",
                            );
                        } else {
                            #[cfg(not(feature = "flutter"))]
                            {
                                let remote_dir = self.get_option("remote_dir".to_string());
                                let show_hidden =
                                    !self.get_option("remote_show_hidden".to_string()).is_empty();
                                self.read_remote_dir(remote_dir, show_hidden);
                            }
                        }
                    } else if !self.is_terminal() {
                        self.msgbox(
                            "success",
                            "Successful",
                            "Connected, waiting for image...",
                            "",
                        );
                    }
                }
            }
        } else {
            log::error!("selected invalid sid: {}", sid);
        }
    }

    #[inline]
    pub fn quick_launch_request(&self, request: String) {
        if request.len() > 32768 || self.lc.read().map(|lc| lc.get_toggle_option("view-only")).unwrap_or(true) {
            return;
        }
        let mut misc = Misc::new();
        misc.set_quick_launch_request(request);
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }

    pub fn request_init_msgs(&self, display: usize) {
        self.send_message_query(display);
    }

    fn send_message_query(&self, display: usize) {
        let mut misc = Misc::new();
        misc.set_message_query(MessageQuery {
            switch_display: display as _,
            ..Default::default()
        });
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }

    pub fn get_conn_token(&self) -> Option<String> {
        self.lc.read().unwrap().get_conn_token()
    }

}

pub trait InvokeUiSession: Send + Sync + Clone + 'static + Sized + Default {
    fn quick_launch_response(&self, _response: String) {}
    fn set_cursor_data(&self, cd: CursorData);
    fn set_cursor_id(&self, id: String);
    fn set_cursor_position(&self, cp: CursorPosition);
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, cursor_embedded: bool, scale: f64);
    fn switch_display(&self, display: &SwitchDisplay);
    fn set_peer_info(&self, peer_info: &PeerInfo); // flutter
    fn set_displays(&self, displays: &Vec<DisplayInfo>);
    fn set_platform_additions(&self, data: &str);
    fn on_connected(&self, conn_type: ConnType);
    fn update_privacy_mode(&self);
    fn set_permission(&self, name: &str, value: bool);
    fn close_success(&self);
    fn update_quality_status(&self, qs: QualityStatus);
    fn set_connection_type(&self, is_secured: bool, direct: bool, stream_type: &str);
    fn set_fingerprint(&self, fingerprint: String);
    fn job_error(&self, id: i32, err: String, file_num: i32);
    fn job_done(&self, id: i32, file_num: i32);
    fn clear_all_jobs(&self);
    fn new_message(&self, msg: String);
    fn update_transfer_list(&self);
    fn load_last_job(&self, cnt: i32, job_json: &str, auto_start: bool);
    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    );
    fn confirm_delete_files(&self, id: i32, i: i32, name: String);
    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        to: String,
        is_upload: bool,
        is_identical: bool,
    );
    fn update_block_input_state(&self, on: bool);
    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64);
    fn adapt_size(&self);
    fn on_rgba(&self, display: usize, rgba: &mut scrap::ImageRgb);
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool);
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String);
    fn cancel_msgbox(&self, tag: &str);
    fn switch_back(&self, id: &str);
    fn portable_service_running(&self, running: bool);
    fn on_voice_call_started(&self);
    fn on_voice_call_closed(&self, reason: &str);
    fn on_voice_call_waiting(&self);
    fn on_voice_call_incoming(&self);
    fn get_rgba(&self, display: usize) -> *const u8;
    fn next_rgba(&self, display: usize);
    #[cfg(all(feature = "vram", feature = "flutter"))]
    fn on_texture(&self, display: usize, texture: *mut c_void);
    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>);
    fn set_current_display(&self, disp_idx: i32);
    #[cfg(feature = "flutter")]
    fn is_multi_ui_session(&self) -> bool;
    fn update_record_status(&self, start: bool);
    fn update_empty_dirs(&self, _res: ReadEmptyDirsResponse) {}
    fn handle_screenshot_resp(&self, sid: String, msg: String);
    fn handle_terminal_response(&self, response: TerminalResponse);
}

impl<T: InvokeUiSession> Deref for Session<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ui_handler
    }
}

impl<T: InvokeUiSession> DerefMut for Session<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui_handler
    }
}

impl<T: InvokeUiSession> FileManager for Session<T> {}

#[async_trait]
impl<T: InvokeUiSession> Interface for Session<T> {
    fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>> {
        return self.lc.clone();
    }

    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str) {
        let direct = self.lc.read().unwrap().direct;
        let received = self.lc.read().unwrap().received;
        let retry_for_relay = direct == Some(true) && !received;
        let retry = check_if_retry(msgtype, title, text, retry_for_relay);
        self.ui_handler.msgbox(msgtype, title, text, link, retry);
    }

    fn handle_login_error(&self, err: &str) -> bool {
        handle_login_error(self.lc.clone(), err, self)
    }

    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>) {
        self.ui_handler.set_multiple_windows_session(sessions);
    }

    fn handle_peer_info(&self, mut pi: PeerInfo) {
        log::debug!("handle_peer_info :{:?}", pi);
        self.lc.write().unwrap().peer_info = Some(pi.clone());
        if pi.current_display as usize >= pi.displays.len() {
            pi.current_display = 0;
        }
        if get_version_number(&pi.version) < get_version_number("1.1.10") {
            self.set_permission("restart", false);
        }
        if self.is_file_transfer() {
            if pi.username.is_empty() && pi.windows_sessions.sessions.is_empty() {
                self.on_error("No active console user logged on, please connect and logon first.");
                return;
            }
        } else if !self.is_port_forward() && !self.is_terminal() {
            if pi.displays.is_empty() {
                self.lc.write().unwrap().handle_peer_info(&pi);
                self.update_privacy_mode();
                let msg = if self.is_view_camera() {
                    "No cameras"
                } else {
                    "No displays"
                };
                self.msgbox("error", "Error", msg, "");
                return;
            }
            if !self.is_view_camera() {
                self.try_change_init_resolution(pi.current_display);
                let p = self.lc.read().unwrap().should_auto_login();
                if !p.is_empty() {
                    input_os_password(p, true, self.clone());
                }
            }
            let current = &pi.displays[pi.current_display as usize];
            self.set_display(
                current.x,
                current.y,
                current.width,
                current.height,
                current.cursor_embedded,
                current.scale,
            );
        }
        self.update_privacy_mode();
        // Clear audit_guid when connection is established successfully
        *self.audit_guid.lock().unwrap() = String::new();
        *self.last_audit_note.lock().unwrap() = String::new();
        // Save recent peers, then push event to flutter. So flutter can refresh peer page.
        self.lc.write().unwrap().handle_peer_info(&pi);
        self.set_peer_info(&pi);
        if self.is_file_transfer() {
            self.close_success();
        } else if !self.is_port_forward() && !self.is_terminal() {
            self.msgbox(
                "success",
                "Successful",
                "Connected, waiting for image...",
                "",
            );
        }
        self.on_connected(self.lc.read().unwrap().conn_type);
        #[cfg(windows)]
        {
            let mut path = std::env::temp_dir();
            path.push(self.get_id());
            let path = path.with_extension(crate::get_app_name().to_lowercase());
            std::fs::File::create(&path).ok();
            if let Some(path) = path.to_str() {
                crate::platform::windows::add_recent_document(&path);
            }
        }
        if !pi.windows_sessions.sessions.is_empty() {
            let selected = self
                .lc
                .read()
                .unwrap()
                .selected_windows_session_id
                .to_owned();
            if selected == Some(pi.windows_sessions.current_sid) {
                self.send_selected_session_id(pi.windows_sessions.current_sid.to_string());
            } else {
                self.set_multiple_windows_session(pi.windows_sessions.sessions.clone());
            }
        }
    }

    async fn handle_hash(&self, pass: &str, hash: Hash, peer: &mut Stream) -> bool {
        handle_hash(self.lc.clone(), pass, hash, self, peer).await
    }

    async fn handle_login_from_ui(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
        peer: &mut Stream,
    ) {
        handle_login_from_ui(
            self.lc.clone(),
            os_username,
            os_password,
            password,
            remember,
            peer,
        )
        .await;
    }

    async fn handle_test_delay(&self, t: TestDelay, peer: &mut Stream) {
        if !t.from_client {
            self.update_quality_status(QualityStatus {
                delay: Some(t.last_delay as _),
                target_bitrate: Some(t.target_bitrate as _),
                ..Default::default()
            });
            handle_test_delay(t, peer).await;
        }
    }

    fn swap_modifier_mouse(&self, msg: &mut base::protos::message::MouseEvent) {
        let allow_swap_key = self.get_toggle_option("allow_swap_key".to_string());
        if allow_swap_key {
            msg.modifiers = msg
                .modifiers
                .iter()
                .map(|ck| {
                    let ck = ck.enum_value_or_default();
                    let ck = match ck {
                        ControlKey::Control => ControlKey::Meta,
                        ControlKey::Meta => ControlKey::Control,
                        ControlKey::RControl => ControlKey::Meta,
                        ControlKey::RWin => ControlKey::Control,
                        _ => ck,
                    };
                    hbb_common::protobuf::EnumOrUnknown::new(ck)
                })
                .collect();
        };
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn io_loop<T: InvokeUiSession>(handler: Session<T>, round: u32) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let (sender, receiver) = mpsc::unbounded_channel::<Data>();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
    *handler.sender.write().unwrap() = Some(sender.clone());
    let token = LocalConfig::get_option("access_token");
    let key = crate::get_key(false).await;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if handler.is_port_forward() {
        handler.lc.write().unwrap().port_forward_mux = crate::port_forward::mux_enabled();
        if handler.is_rdp() {
            let port = handler
                .get_option("rdp_port".to_owned())
                .parse::<i32>()
                .unwrap_or(3389);
            std::env::set_var(
                "rdp_username",
                handler.get_option("rdp_username".to_owned()),
            );
            std::env::set_var(
                "rdp_password",
                handler.get_option("rdp_password".to_owned()),
            );
            log::info!("Remote rdp port: {}", port);
            start_one_port_forward(handler, 0, "".to_owned(), port, receiver, &key, &token).await;
        } else if handler.args.len() == 0 {
            let pfs = handler.lc.read().unwrap().port_forwards.clone();
            let mut queues = HashMap::<i32, mpsc::UnboundedSender<Data>>::new();
            for d in pfs {
                sender.send(Data::AddPortForward(d)).ok();
            }
            loop {
                match receiver.recv().await {
                    Some(Data::AddPortForward((port, remote_host, remote_port))) => {
                        if port <= 0 || remote_port <= 0 {
                            continue;
                        }
                        let (sender, receiver) = mpsc::unbounded_channel::<Data>();
                        queues.insert(port, sender);
                        let handler = handler.clone();
                        let key = key.clone();
                        let token = token.clone();
                        tokio::spawn(async move {
                            start_one_port_forward(
                                handler,
                                port,
                                remote_host,
                                remote_port,
                                receiver,
                                &key,
                                &token,
                            )
                            .await;
                        });
                    }
                    Some(Data::RemovePortForward(port)) => {
                        if let Some(s) = queues.remove(&port) {
                            s.send(Data::Close).ok();
                        }
                    }
                    Some(Data::Close) => {
                        break;
                    }
                    Some(d) => {
                        for (_, s) in queues.iter() {
                            s.send(d.clone()).ok();
                        }
                    }
                    _ => {}
                }
            }
        } else {
            let port = handler.args[0].parse::<i32>().unwrap_or(0);
            if handler.args.len() != 3
                || handler.args[2].parse::<i32>().unwrap_or(0) <= 0
                || port <= 0
            {
                handler.on_error("Invalid arguments, usage:<br><br> rustdesk --port-forward remote-id listen-port remote-host remote-port");
            }
            let remote_host = handler.args[1].clone();
            let remote_port = handler.args[2].parse::<i32>().unwrap_or(0);
            start_one_port_forward(
                handler,
                port,
                remote_host,
                remote_port,
                receiver,
                &key,
                &token,
            )
            .await;
        }
        return;
    }
    let mut remote = Remote::new(handler, receiver, sender);
    remote.io_loop(&key, &token, round).await;
    let _ = remote.sync_jobs_status_to_local().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn start_one_port_forward<T: InvokeUiSession>(
    handler: Session<T>,
    port: i32,
    remote_host: String,
    remote_port: i32,
    receiver: mpsc::UnboundedReceiver<Data>,
    key: &str,
    token: &str,
) {
    if let Err(err) = crate::port_forward::listen(
        handler.get_id(),
        handler.password.clone(),
        port,
        handler.clone(),
        receiver,
        key,
        token,
        handler.lc.clone(),
        remote_host,
        remote_port,
    )
    .await
    {
        handler.on_error(&format!("Failed to listen on {}: {}", port, err));
    }
    log::info!("port forward (:{}) exit", port);
}

#[tokio::main(flavor = "current_thread")]
async fn send_note(url: String, id: String, sid: u64, note: String) {
    let body = serde_json::json!({ "id": id, "session_id": sid, "note": note });
    allow_err!(crate::post_request(url, body.to_string(), "").await);
}
