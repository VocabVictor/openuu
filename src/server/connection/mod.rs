#[cfg(target_os = "windows")]
use super::login_failure_check::try_acquire_os_credential_login_gate;
use super::login_failure_check::{
    evaluate_os_credential_policy, record_os_credential_failure, FailureScope,
};
use super::{input_service::*, *};
#[cfg(feature = "unix-file-copy-paste")]
use crate::clipboard::try_empty_clipboard_files;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::clipboard::{update_clipboard, ClipboardSide};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crate::clipboard_file::*;
#[cfg(target_os = "android")]
use crate::keyboard::client::map_key_to_control_key;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::platform::WallPaperRemover;
#[cfg(windows)]
use crate::portable_service::client as portable_client;
use crate::{
    client::{
        new_voice_call_request, new_voice_call_response, start_audio_thread, MediaData, MediaSender,
    },
    display_service, ipc, privacy_mode, video_service, VERSION,
};
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::{common::DEVICE_NAME, flutter::connection_manager::start_channel};
use cidr_utils::cidr::IpCidr;
#[cfg(target_os = "android")]
use hbb_common::protobuf::EnumOrUnknown;
use hbb_common::{
    config::{
        self, decode_permanent_password_h1_from_storage, decode_preset_password_h1_from_storage,
        local_permanent_password_storage_is_usable_for_auth,
        preset_permanent_password_storage_is_usable_for_auth, Config, TrustedDevice,
    },
    futures::{SinkExt, StreamExt},
    get_time, get_version_number,
    password_security::{self as password, ApproveMode},
    sha2::{Digest, Sha256},
    sleep, timeout,
    tokio::{
        net::TcpStream,
        sync::mpsc,
        time::{self, Duration, Instant},
    },
    tokio_util::codec::{BytesCodec, Framed},
};
use base::{
    config::keys,
    fs::{self, can_enable_overwrite_detection, JobType},
    message_proto::{option_message::BoolOption, permission_info::Permission},
};
#[cfg(any(target_os = "android", target_os = "ios"))]
use scrap::android::{call_main_service_key_event, call_main_service_pointer_input};
use scrap::camera;
use serde_derive::Serialize;
use serde_json::{json, value::Value};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::atomic::Ordering;
use std::{
    collections::HashSet,
    net::Ipv6Addr,
    num::NonZeroI64,
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicI64, mpsc as std_mpsc},
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use system_shutdown;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};

#[cfg(windows)]
use crate::virtual_display_manager;
pub type Sender = mpsc::UnboundedSender<(Instant, Arc<Message>)>;

const FAILURE_IDX_ID_WHITELIST: usize = 2;
// How long a rejection counts, so also how long a blocked address stays blocked. Longer
// throttles enumeration harder; shorter limits collateral on whitelisted neighbours.
const ID_WHITELIST_FAILURE_DECAY_MINUTES: i32 = 10;

lazy_static::lazy_static! {
    // [0] password, [1] 2FA, [2] ID whitelist.
    // Bucket 2 is separate so its rejections do not touch the password / 2FA budgets. It is
    // decayed in `check_id_whitelist` and cleared on auth, never on a bare id match.
    static ref LOGIN_FAILURES: [Arc::<Mutex<HashMap<String, (i32, i32, i32)>>>; 3] = Default::default();
    static ref SESSIONS: Arc::<Mutex<HashMap<SessionKey, Session>>> = Default::default();
    static ref ALIVE_CONNS: Arc::<Mutex<Vec<i32>>> = Default::default();
    pub static ref AUTHED_CONNS: Arc::<Mutex<Vec<AuthedConn>>> = Default::default();
    pub static ref CONTROL_PERMISSIONS_ARRAY: Arc::<Mutex<Vec<(i32, ControlPermissions)>>> = Default::default();
    static ref WAKELOCK_SENDER: Arc::<Mutex<std::sync::mpsc::Sender<(usize, usize)>>> = Arc::new(Mutex::new(start_wakelock_thread()));
    static ref WAKELOCK_KEEP_AWAKE_OPTION: Arc::<Mutex<Option<bool>>> = Default::default();
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const SWITCH_SIDES_UUID_TTL: Duration = Duration::from_secs(10);

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
lazy_static::lazy_static! {
    static ref SWITCH_SIDES_UUID: Arc::<Mutex<HashMap<String, (Instant, uuid::Uuid)>>> = Default::default();
    static ref PENDING_SWITCH_SIDES_UUID: Arc::<Mutex<HashMap<String, (Instant, uuid::Uuid, bool)>>> = Default::default();
}

#[cfg(target_os = "windows")]
const TERMINAL_OS_LOGIN_FAILED_MSG: &str = "Incorrect username or password.";

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    // Avoid data-dependent early exits.
    let mut x: u8 = 0;
    for i in 0..a.len() {
        x |= a[i] ^ b[i];
    }
    x == 0
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn should_use_terminal_os_login_scope(is_terminal: bool, os_login_username: &str) -> bool {
    cfg!(target_os = "windows") && is_terminal && !os_login_username.trim().is_empty()
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
lazy_static::lazy_static! {
    static ref WALLPAPER_REMOVER: Arc<Mutex<Option<WallPaperRemover>>> = Default::default();
}
pub static CLICK_TIME: AtomicI64 = AtomicI64::new(0);
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub static MOUSE_MOVE_TIME: AtomicI64 = AtomicI64::new(0);

mod types;
pub use types::*;
mod conn_struct;
pub use conn_struct::*;
mod conn_inner;
mod consts;
use consts::*;
mod start;
mod loops;
mod access_checks;
mod audit;
mod port_forward;
mod logon_response;
mod subscriptions;
mod cm_and_input;
mod password_check;
mod login_scope;
mod on_message;
mod terminal_login;
mod failures;
mod display;
mod voice_options;
mod privacy_close;
mod file_transfer;
mod housekeeping;
mod scope;
mod scope_rules;
mod clipboard_terminal;
mod switch_sides;
pub use switch_sides::*;
mod ipc_start;
use ipc_start::*;

impl Connection {
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AlarmAuditType {
    IpWhitelist = 0,
    ExceedThirtyAttempts = 1,
    SixAttemptsWithinOneMinute = 2,
    // ExceedThirtyLoginAttempts = 3,
    // MultipleLoginsAttemptsWithinOneMinute = 4,
    // MultipleLoginsAttemptsWithinOneHour = 5,
    ExceedIPv6PrefixAttempts = 6,
    TerminalOsLoginBackoff = 7,
    TerminalOsLoginConcurrency = 8,
    SessionScopeViolation = 9,
    IdWhitelist = 10,
}

pub enum FileAuditType {
    RemoteSend = 0,
    RemoteReceive = 1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileActionLog {
    id: i32,
    conn_id: i32,
    path: String,
    dir: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileRenameLog {
    conn_id: i32,
    path: String,
    new_name: String,
}

struct FileRemoveLogControl {
    conn_id: i32,
    instant: Instant,
    removed_files: Vec<FileRemoveFile>,
    removed_dirs: Vec<FileRemoveDir>,
}

impl FileRemoveLogControl {
    fn new(conn_id: i32) -> Self {
        FileRemoveLogControl {
            conn_id,
            instant: Instant::now(),
            removed_files: vec![],
            removed_dirs: vec![],
        }
    }

    fn on_remove_file(&mut self, f: FileRemoveFile) -> Option<ipc::Data> {
        self.instant = Instant::now();
        self.removed_files.push(f.clone());
        Some(ipc::Data::FileTransferLog((
            "remove".to_string(),
            serde_json::to_string(&FileActionLog {
                id: f.id,
                conn_id: self.conn_id,
                path: f.path,
                dir: false,
            })
            .unwrap_or_default(),
        )))
    }

    fn on_remove_dir(&mut self, d: FileRemoveDir) -> Option<ipc::Data> {
        self.instant = Instant::now();
        let direct_child = |parent: &str, child: &str| {
            PathBuf::from(child).parent().map(|x| x.to_path_buf()) == Some(PathBuf::from(parent))
        };
        self.removed_files
            .retain(|f| !direct_child(&f.path, &d.path));
        self.removed_dirs
            .retain(|x| !direct_child(&d.path, &x.path));
        if !self
            .removed_dirs
            .iter()
            .any(|x| direct_child(&x.path, &d.path))
        {
            self.removed_dirs.push(d.clone());
        }
        Some(ipc::Data::FileTransferLog((
            "remove".to_string(),
            serde_json::to_string(&FileActionLog {
                id: d.id,
                conn_id: self.conn_id,
                path: d.path,
                dir: true,
            })
            .unwrap_or_default(),
        )))
    }

    fn on_timer(&mut self) -> Vec<ipc::Data> {
        if self.instant.elapsed().as_secs() < 1 {
            return vec![];
        }
        let mut v: Vec<ipc::Data> = vec![];
        self.removed_files
            .drain(..)
            .map(|f| {
                v.push(ipc::Data::FileTransferLog((
                    "remove".to_string(),
                    serde_json::to_string(&FileActionLog {
                        id: f.id,
                        conn_id: self.conn_id,
                        path: f.path,
                        dir: false,
                    })
                    .unwrap_or_default(),
                )));
            })
            .count();
        self.removed_dirs
            .drain(..)
            .map(|d| {
                v.push(ipc::Data::FileTransferLog((
                    "remove".to_string(),
                    serde_json::to_string(&FileActionLog {
                        id: d.id,
                        conn_id: self.conn_id,
                        path: d.path,
                        dir: true,
                    })
                    .unwrap_or_default(),
                )));
            })
            .count();
        v
    }
}

fn start_wakelock_thread() -> std::sync::mpsc::Sender<(usize, usize)> {
    // Check if we should keep awake during incoming sessions
    use crate::platform::{get_wakelock, WakeLock};
    let (tx, rx) = std::sync::mpsc::channel::<(usize, usize)>();
    std::thread::spawn(move || {
        let mut wakelock: Option<WakeLock> = None;
        let mut last_display = false;
        loop {
            match rx.recv() {
                Ok((conn_count, remote_count)) => {
                    let keep_awake = config::Config::get_bool_option(
                        keys::OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS,
                    );
                    *WAKELOCK_KEEP_AWAKE_OPTION.lock().unwrap() = Some(keep_awake);
                    if conn_count == 0 || !keep_awake {
                        if wakelock.is_some() {
                            wakelock = None;
                            log::info!("drop wakelock");
                        }
                    } else {
                        let mut display = remote_count > 0;
                        if let Some(_w) = wakelock.as_mut() {
                            if display != last_display {
                                #[cfg(any(target_os = "windows", target_os = "macos"))]
                                {
                                    log::info!("set wakelock display to {display}");
                                    if let Err(e) = _w.set_display(display) {
                                        log::error!(
                                            "failed to set wakelock display to {display}: {e:?}"
                                        );
                                    }
                                }
                            }
                        } else {
                            if cfg!(target_os = "linux") {
                                display = true;
                            }
                            wakelock = Some(get_wakelock(display));
                        }
                        last_display = display;
                    }
                }
                Err(e) => {
                    log::error!("wakelock receive error: {e:?}");
                    break;
                }
            }
        }
    });
    tx
}

#[cfg(windows)]
pub struct PortableState {
    pub last_uac: bool,
    pub last_foreground_window_elevated: bool,
    pub last_running: Option<bool>,
    pub is_installed: bool,
}

#[cfg(windows)]
impl Default for PortableState {
    fn default() -> Self {
        Self {
            is_installed: crate::platform::is_installed(),
            last_uac: Default::default(),
            last_foreground_window_elevated: Default::default(),
            last_running: Default::default(),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        self.release_pressed_modifiers();

        if let Some(s) = self.terminal_generic_service.as_ref() {
            s.join();
        }

        #[cfg(target_os = "windows")]
        if let Some(TerminalUserToken::CurrentLogonUser(token)) = self.terminal_user_token.take() {
            if token.as_raw() != 0 {
                unsafe {
                    hbb_common::allow_err!(CloseHandle(HANDLE(token.as_raw() as _)));
                };
            }
        }
    }
}

extern "C" fn connection_shutdown_hook() {
    // https://stackoverflow.com/questions/35980148/why-does-an-atexit-handler-panic-when-it-accesses-stdout
    // Please make sure there is no print in the call stack
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        *WALLPAPER_REMOVER.lock().unwrap() = None;
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Default)]
struct Retina {
    displays: Vec<DisplayInfo>,
}

#[cfg(target_os = "macos")]
impl Retina {
    #[inline]
    fn set_displays(&mut self, displays: &Vec<DisplayInfo>) {
        self.displays = displays.clone();
    }

    #[inline]
    fn on_mouse_event(&mut self, e: &mut MouseEvent, current: usize) {
        let evt_type = e.mask & crate::input::MOUSE_TYPE_MASK;
        // Delta-based events do not contain absolute coordinates.
        // Avoid applying Retina coordinate scaling to them.
        if evt_type == crate::input::MOUSE_TYPE_WHEEL
            || evt_type == crate::input::MOUSE_TYPE_TRACKPAD
            || evt_type == crate::input::MOUSE_TYPE_MOVE_RELATIVE
        {
            return;
        }
        let Some(d) = self.displays.get(current) else {
            return;
        };
        let s = d.scale;
        if s > 1.0 && e.x >= d.x && e.y >= d.y && e.x < d.x + d.width && e.y < d.y + d.height {
            e.x = d.x + ((e.x - d.x) as f64 / s) as i32;
            e.y = d.y + ((e.y - d.y) as f64 / s) as i32;
        }
    }

    #[inline]
    fn on_cursor_pos(&mut self, pos: &CursorPosition, current: usize) -> Option<Message> {
        let Some(d) = self.displays.get(current) else {
            return None;
        };
        let s = d.scale;
        if s > 1.0
            && pos.x >= d.x
            && pos.y >= d.y
            && (pos.x - d.x) as f64 * s < d.width as f64
            && (pos.y - d.y) as f64 * s < d.height as f64
        {
            let mut pos = pos.clone();
            pos.x = d.x + ((pos.x - d.x) as f64 * s) as i32;
            pos.y = d.y + ((pos.y - d.y) as f64 * s) as i32;
            let mut msg = Message::new();
            msg.set_cursor_position(pos);
            return Some(msg);
        }
        None
    }
}

/// Get control permission state from CONTROL_PERMISSIONS_ARRAY.
/// Returns: Some(false) if any disable, Some(true) if any enable (and no disable), None if not set.
pub fn get_control_permission_state(
    permission: hbb_common::rendezvous_proto::control_permissions::Permission,
    disable_if_has_disabled: bool,
) -> Option<bool> {
    let control_permissions = CONTROL_PERMISSIONS_ARRAY.lock().unwrap();
    let mut has_enable = false;
    let mut has_disable = false;
    for (_, cp) in control_permissions.iter() {
        match crate::get_control_permission(cp.permissions, permission) {
            Some(false) => has_disable = true,
            Some(true) => has_enable = true,
            None => {}
        }
    }
    if disable_if_has_disabled {
        if has_disable {
            Some(false)
        } else if has_enable {
            Some(true)
        } else {
            None
        }
    } else {
        if has_enable {
            Some(true)
        } else if has_disable {
            Some(false)
        } else {
            None
        }
    }
}

pub struct AuthedConn {
    pub conn_id: i32,
    pub conn_type: AuthConnType,
    pub session_key: SessionKey,
    pub sender: mpsc::UnboundedSender<Data>,
}

mod raii {
    // ALIVE_CONNS: all connections, including unauthorized connections
    // AUTHED_CONNS: all authorized connections
    // CONTROL_PERMISSIONS_ARRAY: all non-None control permissions

    use super::*;
    pub struct ConnectionID(i32);

    impl ConnectionID {
        pub fn new(id: i32) -> Self {
            ALIVE_CONNS.lock().unwrap().push(id);
            Self(id)
        }
    }

    impl Drop for ConnectionID {
        fn drop(&mut self) {
            let mut active_conns_lock = ALIVE_CONNS.lock().unwrap();
            active_conns_lock.retain(|&c| c != self.0);
        }
    }

    pub struct AuthedConnID(i32, AuthConnType);

    impl AuthedConnID {
        pub(super) fn is_newer_session_remote(c: &AuthedConn, id: i32, key: &SessionKey) -> bool {
            c.conn_id > id && c.conn_type == AuthConnType::Remote && &c.session_key == key
        }

        /// Whether a newer remote control connection of this session has replaced this one. A
        /// controlling peer whose link dies reconnects while the connection it left behind runs
        /// on here until its own timeout; locking for that one would lock a session that has
        /// already resumed on its replacement.
        pub fn session_reconnected(id: i32, key: &SessionKey) -> bool {
            let conns = AUTHED_CONNS.lock().unwrap();
            conns
                .iter()
                .any(|c| Self::is_newer_session_remote(c, id, key))
        }

        pub fn new(
            conn_id: i32,
            conn_type: AuthConnType,
            session_key: SessionKey,
            sender: mpsc::UnboundedSender<Data>,
            lr: LoginRequest,
        ) -> Self {
            AUTHED_CONNS.lock().unwrap().push(AuthedConn {
                conn_id,
                conn_type,
                session_key,
                sender,
            });
            Self::check_wake_lock();
            use std::sync::Once;
            static _ONCE: Once = Once::new();
            _ONCE.call_once(|| {
                shutdown_hooks::add_shutdown_hook(connection_shutdown_hook);
            });
            if conn_type == AuthConnType::Remote || conn_type == AuthConnType::ViewCamera {
                video_service::VIDEO_QOS
                    .lock()
                    .unwrap()
                    .on_connection_open(conn_id);
            }
            Self(conn_id, conn_type)
        }

        fn check_wake_lock() {
            let conn_count = AUTHED_CONNS.lock().unwrap().len();
            let remote_count = AUTHED_CONNS
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.conn_type == AuthConnType::Remote)
                .count();
            allow_err!(WAKELOCK_SENDER
                .lock()
                .unwrap()
                .send((conn_count, remote_count)));
        }

        pub fn check_wake_lock_on_setting_changed() {
            let current =
                config::Config::get_bool_option(keys::OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS);
            let cached = *WAKELOCK_KEEP_AWAKE_OPTION.lock().unwrap();
            if cached != Some(current) {
                Self::check_wake_lock();
            }
        }

        #[cfg(windows)]
        pub fn non_port_forward_conn_count() -> usize {
            AUTHED_CONNS
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.conn_type != AuthConnType::PortForward)
                .count()
        }

        pub fn check_remove_session(conn_id: i32, key: SessionKey) {
            let mut lock = SESSIONS.lock().unwrap();
            let contains = lock.contains_key(&key);
            if contains {
                // No two remote connections with the same session key, just for ensure.
                let is_remote = AUTHED_CONNS
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|c| c.conn_id == conn_id && c.conn_type == AuthConnType::Remote);
                // If there are 2 connections with the same peer_id and session_id, a remote connection and a file transfer or port forward connection,
                // If any of the connections is closed allowing retry, this will not be called;
                // If the file transfer/port forward connection is closed with no retry, the session should be kept for remote control menu action;
                // If the remote connection is closed with no retry, keep the session is not reasonable in case there is a retry button in the remote side, and ignore network fluctuations.
                let another_remote = AUTHED_CONNS.lock().unwrap().iter().any(|c| {
                    c.conn_id != conn_id
                        && c.session_key == key
                        && c.conn_type == AuthConnType::Remote
                });
                if is_remote || !another_remote {
                    lock.remove(&key);
                    log::info!("remove session");
                } else {
                    // Keep the session if there is another remote connection with same peer_id and session_id.
                    log::info!("skip remove session");
                }
            }
        }

        pub fn update_or_insert_session(
            key: SessionKey,
            password: Option<String>,
            tfa: Option<bool>,
        ) {
            let mut lock = SESSIONS.lock().unwrap();
            let session = lock.get_mut(&key);
            if let Some(session) = session {
                if let Some(password) = password {
                    session.random_password = password;
                }
                if let Some(tfa) = tfa {
                    session.tfa = tfa;
                }
            } else {
                lock.insert(
                    key,
                    Session {
                        random_password: password.unwrap_or_default(),
                        tfa: tfa.unwrap_or_default(),
                        last_recv_time: Arc::new(Mutex::new(Instant::now())),
                    },
                );
            }
        }

        pub fn set_session_2fa(key: SessionKey) {
            let mut lock = SESSIONS.lock().unwrap();
            let session = lock.get_mut(&key);
            if let Some(session) = session {
                session.tfa = true;
            } else {
                lock.insert(
                    key,
                    Session {
                        last_recv_time: Arc::new(Mutex::new(Instant::now())),
                        random_password: "".to_owned(),
                        tfa: true,
                    },
                );
            }
        }

        pub fn conn_type(&self) -> AuthConnType {
            self.1
        }
    }

    impl Drop for AuthedConnID {
        fn drop(&mut self) {
            if self.1 == AuthConnType::Remote || self.1 == AuthConnType::ViewCamera {
                scrap::codec::Encoder::update(scrap::codec::EncodingUpdate::Remove(self.0));
                video_service::VIDEO_QOS
                    .lock()
                    .unwrap()
                    .on_connection_close(self.0);
            }
            // Clear per-connection state to avoid stale behavior if conn ids are reused.
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            clear_relative_mouse_active(self.0);
            AUTHED_CONNS.lock().unwrap().retain(|c| c.conn_id != self.0);
            let remote_count = AUTHED_CONNS
                .lock()
                .unwrap()
                .iter()
                .filter(|c| c.conn_type == AuthConnType::Remote)
                .count();
            if remote_count == 0 {
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                {
                    *WALLPAPER_REMOVER.lock().unwrap() = None;
                }
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                display_service::restore_resolutions();
                #[cfg(windows)]
                let _ = virtual_display_manager::reset_all();
                #[cfg(target_os = "linux")]
                scrap::wayland::pipewire::try_close_session();
            }
            Self::check_wake_lock();
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                use crate::whiteboard;
                whiteboard::unregister_whiteboard(whiteboard::get_key_cursor(self.0));
            }
        }
    }

    pub struct ControlPermissionsID {
        id: i32,
        control_permissions: Option<ControlPermissions>,
    }

    impl Drop for ControlPermissionsID {
        fn drop(&mut self) {
            if self.control_permissions.is_some() {
                let mut lock = CONTROL_PERMISSIONS_ARRAY.lock().unwrap();
                lock.retain(|(conn_id, _)| *conn_id != self.id);
            }
        }
    }
    impl ControlPermissionsID {
        pub fn new(id: i32, control_permissions: &Option<ControlPermissions>) -> Self {
            if let Some(s) = control_permissions {
                CONTROL_PERMISSIONS_ARRAY
                    .lock()
                    .unwrap()
                    .push((id, s.clone()));
            }
            Self {
                id,
                control_permissions: control_permissions.clone(),
            }
        }
    }
}

// An empty whitelist allows everyone.
//
// A peer connecting across servers reports `<its id>@<its own server>` (see
// `create_login_msg`), so the bare id is matched as well. That suffix is self-asserted and
// unsigned, so matching only the full form would reject the honest cross-server peer while
// an attacker just reports the bare id: it can produce false rejects but no true ones.
fn id_whitelist_allows(id_whitelist: &[String], my_id: &str) -> bool {
    if id_whitelist.is_empty() {
        return true;
    }
    let bare_id = my_id.split('@').next().unwrap_or(my_id);
    id_whitelist
        .iter()
        .any(|x| wildcard_match(x, my_id) || wildcard_match(x, bare_id))
}

// Drop `keys` whose last failure (`.0`, in minutes) is at least `window` old. A backwards
// clock gives a negative age and keeps the entry, so it never widens access.
fn decay_stale_failures(
    failures: &mut HashMap<String, (i32, i32, i32)>,
    keys: &[String],
    now: i32,
    window: i32,
) {
    for key in keys {
        if failures
            .get(key)
            .is_some_and(|v| now.saturating_sub(v.0) >= window)
        {
            failures.remove(key);
        }
    }
}

// Unconditionally forget `keys`, unlike `update_failure`'s remove path which requires the
// per-address entry to exist.
fn clear_failures(failures: &mut HashMap<String, (i32, i32, i32)>, keys: &[String]) {
    for key in keys {
        failures.remove(key);
    }
}

// Simple glob matching for the ID whitelist: '*' matches any sequence of characters
// (including the empty one), '?' matches exactly one character. Case-insensitive.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.trim().to_lowercase().chars().collect();
    let t: Vec<char> = text.trim().to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi + 1, ti));
            pi += 1;
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if let Some((sp, st)) = star {
            pi = sp;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod test {
    #[allow(unused)]
    use super::*;

    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn test_pending_switch_sides_uuid_is_claimed_once() {
        let id = uuid::Uuid::new_v4().to_string();
        let uuid = uuid::Uuid::new_v4();
        let other_uuid = uuid::Uuid::new_v4();
        assert!(insert_pending_switch_sides_uuid(id.clone(), uuid.clone()));

        assert!(!insert_pending_switch_sides_uuid(id.clone(), uuid.clone()));
        assert!(has_pending_switch_sides_uuid(&id, &uuid));
        assert!(!has_pending_switch_sides_uuid(&id, &other_uuid));
        assert!(!claim_pending_switch_sides_uuid("other-peer", &uuid));
        assert!(!claim_pending_switch_sides_uuid(&id, &other_uuid));
        assert!(claim_pending_switch_sides_uuid(&id, &uuid));
        assert!(!has_pending_switch_sides_uuid(&id, &uuid));
        assert!(!claim_pending_switch_sides_uuid(&id, &uuid));
        assert!(!insert_pending_switch_sides_uuid(id, uuid));
    }

    #[test]
    fn login_scope_latches_session_scope_across_login_retries() {
        let port_forward = |host: &str| {
            let mut lr = LoginRequest::new();
            lr.my_id = "peer".to_owned();
            lr.set_port_forward(PortForward {
                host: host.to_owned(),
                port: 3389,
                ..Default::default()
            });
            lr
        };
        let first = port_forward("localhost");
        let scope = |lr: &LoginRequest| Connection::login_scope_digest(lr);

        // A retry may carry new credentials, profile data, options, and unknown fields.
        let mut retry = port_forward("localhost");
        retry.password = "secret".into();
        retry.hwid = "hwid".into();
        retry.os_login = Some(OSLogin {
            username: "admin".to_owned(),
            ..Default::default()
        })
        .into();
        retry.my_name = "New Display Name".to_owned();
        retry.avatar = "data:image/png;base64,AAAA".to_owned();
        retry
            .special_fields
            .mut_unknown_fields()
            .add_varint(9999, 1);
        assert_eq!(scope(&first), scope(&retry));

        // It may not change the controller identity, move the target, or switch type.
        let mut rotated_id = first.clone();
        rotated_id.my_id = "rotated-id".to_owned();
        assert_ne!(scope(&first), scope(&rotated_id));
        assert_ne!(scope(&first), scope(&port_forward("10.0.0.5")));
        let mut moved_port = port_forward("localhost");
        moved_port.mut_port_forward().port = 22;
        assert_ne!(scope(&first), scope(&moved_port));
        let terminal = |service_id: &str| {
            let mut lr = LoginRequest::new();
            lr.my_id = "peer".to_owned();
            lr.set_terminal(Terminal {
                service_id: service_id.to_owned(),
                ..Default::default()
            });
            lr
        };
        assert_ne!(scope(&first), scope(&terminal("")));
        assert_ne!(scope(&terminal("a")), scope(&terminal("b")));
    }

    #[test]
    fn test_wildcard_match() {
        // Exact match.
        assert!(wildcard_match("123456789", "123456789"));
        assert!(!wildcard_match("123456789", "123456780"));
        assert!(!wildcard_match("12345678", "123456789"));
        assert!(!wildcard_match("123456789", "12345678"));
        // Case-insensitive.
        assert!(wildcard_match("MyCustomId", "mycustomid"));
        // '*' matches any sequence.
        assert!(wildcard_match("*", "123456789"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("*", "*abc"));
        assert!(wildcard_match("123*", "123456789"));
        assert!(wildcard_match("123*", "123"));
        assert!(wildcard_match("12*", "12*9"));
        assert!(!wildcard_match("123*", "124456789"));
        assert!(wildcard_match("*789", "123456789"));
        assert!(wildcard_match("1*9", "123456789"));
        assert!(wildcard_match("1*4*9", "123456789"));
        assert!(!wildcard_match("1*4*9", "123456780"));
        assert!(wildcard_match("*456*", "123456789"));
        // '?' matches exactly one character.
        assert!(wildcard_match("12345678?", "123456789"));
        assert!(!wildcard_match("123456789?", "123456789"));
        assert!(wildcard_match("???456???", "123456789"));
        assert!(wildcard_match("1?3*7?9", "123456789"));
        // Whitespace around entries is ignored.
        assert!(wildcard_match(" 123456789 ", "123456789"));
    }

    #[test]
    fn test_decay_stale_failures() {
        let entry = |minute: i32| (minute, 1, 40);
        let keys = ["ip".to_string(), "p64".to_string(), "absent".to_string()];
        let mut m: HashMap<String, (i32, i32, i32)> = HashMap::new();
        m.insert("ip".to_string(), entry(100));
        m.insert("p64".to_string(), entry(160));
        m.insert("untouched".to_string(), entry(100));

        // Exactly at the window: forgotten. Still inside it: kept.
        decay_stale_failures(&mut m, &keys, 160, 60);
        assert!(!m.contains_key("ip"));
        assert!(m.contains_key("p64"));
        // Keys that were not passed in are never visited, absent ones are a no-op.
        assert!(m.contains_key("untouched"));

        // One minute short of the window keeps the entry.
        decay_stale_failures(&mut m, &keys, 219, 60);
        assert!(m.contains_key("p64"));
        decay_stale_failures(&mut m, &keys, 220, 60);
        assert!(!m.contains_key("p64"));

        // A clock that jumped backwards must not drop anything.
        m.insert("ip".to_string(), entry(500));
        decay_stale_failures(&mut m, &keys, 0, 60);
        assert!(m.contains_key("ip"));
    }

    #[test]
    fn test_clear_failures_drops_shared_prefixes() {
        // On IPv6 a whitelisted peer usually has no entry of its own, while the shared
        // prefixes that block it do. Clearing must not depend on the per-address entry.
        let mut m: HashMap<String, (i32, i32, i32)> = HashMap::new();
        m.insert("p64".to_string(), (100, 1, 55));
        m.insert("p56".to_string(), (100, 1, 75));
        m.insert("p48".to_string(), (100, 1, 95));
        m.insert("someone-else".to_string(), (100, 1, 95));
        let keys = ["ip", "p64", "p56", "p48"].map(|k| k.to_string());

        clear_failures(&mut m, &keys);

        for key in ["p64", "p56", "p48"] {
            assert!(!m.contains_key(key), "{key} should have been cleared");
        }
        // Keys belonging to other peers are left alone.
        assert!(m.contains_key("someone-else"));
    }

    #[test]
    fn test_id_whitelist_allows() {
        let list = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();

        // An empty whitelist allows everyone.
        assert!(id_whitelist_allows(&[], "123456789"));

        // Same server: the peer reports a bare id.
        assert!(id_whitelist_allows(&list(&["123456789"]), "123456789"));
        assert!(!id_whitelist_allows(&list(&["123456789"]), "987654321"));

        // Cross server: the peer appends its own server, which must not reject it.
        assert!(id_whitelist_allows(
            &list(&["123456789"]),
            "123456789@example.com:21116"
        ));
        // Cross server from web, whose server is a WebSocket URI.
        assert!(id_whitelist_allows(
            &list(&["123456789"]),
            "123456789@wss://example.com:21118/ws/id"
        ));
        // A different id is still rejected, suffix or not.
        assert!(!id_whitelist_allows(
            &list(&["123456789"]),
            "987654321@example.com:21116"
        ));

        // An entry pinned to one server keeps matching that exact form.
        assert!(id_whitelist_allows(
            &list(&["123456789@example.com:21116"]),
            "123456789@example.com:21116"
        ));
        assert!(!id_whitelist_allows(
            &list(&["123456789@example.com:21116"]),
            "123456789@other.com:21116"
        ));
        // ... and no longer matches the bare id, which is the point of pinning.
        assert!(!id_whitelist_allows(
            &list(&["123456789@example.com:21116"]),
            "123456789"
        ));

        // Wildcards keep working on both forms.
        assert!(id_whitelist_allows(&list(&["abc*"]), "abcdef"));
        assert!(id_whitelist_allows(
            &list(&["abc*"]),
            "abcdef@example.com:21116"
        ));
        assert!(id_whitelist_allows(
            &list(&["*"]),
            "123456789@example.com:21116"
        ));

        // Any entry of the list is enough.
        assert!(id_whitelist_allows(
            &list(&["111111111", "123456789", "222222222"]),
            "123456789@example.com:21116"
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn retina() {
        let mut retina = Retina {
            displays: vec![DisplayInfo {
                x: 10,
                y: 10,
                width: 1000,
                height: 1000,
                scale: 2.0,
                ..Default::default()
            }],
        };
        let mut mouse: MouseEvent = MouseEvent {
            x: 510,
            y: 510,
            ..Default::default()
        };
        retina.on_mouse_event(&mut mouse, 0);
        assert_eq!(mouse.x, 260);
        assert_eq!(mouse.y, 260);
        let pos = CursorPosition {
            x: 260,
            y: 260,
            ..Default::default()
        };
        let msg = retina.on_cursor_pos(&pos, 0).unwrap();
        let pos = msg.cursor_position();
        assert_eq!(pos.x, 510);
        assert_eq!(pos.y, 510);
    }

    #[test]
    fn ipv6() {
        assert!(Ipv6Addr::from_str("::1").is_ok());
        assert!(Ipv6Addr::from_str("127.0.0.1").is_err());
        assert!(Ipv6Addr::from_str("0").is_err());
    }

    fn msg(set: impl FnOnce(&mut Message)) -> Message {
        let mut msg = Message::new();
        set(&mut msg);
        msg
    }

    fn misc_msg(set: impl FnOnce(&mut Misc)) -> Message {
        msg(|msg| {
            let mut misc = Misc::new();
            set(&mut misc);
            msg.set_misc(misc);
        })
    }

    fn option_msg(set: impl FnOnce(&mut OptionMessage)) -> Message {
        misc_msg(|misc| {
            let mut option = OptionMessage::new();
            set(&mut option);
            misc.set_option(option);
        })
    }

    fn set_supported_decoding(option: &mut OptionMessage) {
        option.supported_decoding = hbb_common::protobuf::MessageField::some(Default::default());
    }

    fn assert_scopes(
        conn_type: AuthConnType,
        cases: impl IntoIterator<Item = (Message, Option<&'static str>)>,
    ) {
        for (msg, expected) in cases {
            assert_eq!(
                Connection::authorized_message_scope_violation(conn_type, &msg),
                expected
            );
        }
    }

    #[test]
    fn session_scope_allows_only_messages_for_authenticated_session_type() {
        let cases = [
            (
                AuthConnType::FileTransfer,
                vec![
                    (msg(|m| m.set_file_action(FileAction::new())), None),
                    (msg(|m| m.set_file_response(FileResponse::new())), None),
                    (msg(|m| m.set_login_request(LoginRequest::new())), None),
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        Some("screenshot_request"),
                    ),
                    (
                        misc_msg(|m| m.set_capture_displays(CaptureDisplays::new())),
                        Some("misc.capture_displays"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                    (msg(|m| m.set_clipboard(Clipboard::new())), None),
                    (
                        msg(|m| m.set_multi_clipboards(MultiClipboards::new())),
                        None,
                    ),
                    (misc_msg(|m| m.set_refresh_video(true)), None),
                    (misc_msg(|m| m.set_refresh_video_display(0)), None),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default())
                        }),
                        None,
                    ),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default());
                            o.disable_audio = BoolOption::Yes.into();
                        }),
                        Some("misc.option"),
                    ),
                    (
                        msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                        Some("port_forward_channel"),
                    ),
                ],
            ),
            (
                AuthConnType::Terminal,
                vec![
                    (msg(|m| m.set_terminal_action(TerminalAction::new())), None),
                    (
                        option_msg(|o| o.terminal_persistent = BoolOption::Yes.into()),
                        None,
                    ),
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        Some("screenshot_request"),
                    ),
                    (
                        msg(|m| m.set_file_action(FileAction::new())),
                        Some("file_action"),
                    ),
                    (
                        misc_msg(|m| m.set_toggle_privacy_mode(TogglePrivacyMode::new())),
                        Some("misc.toggle_privacy_mode"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                    (misc_msg(|m| m.set_chat_message(ChatMessage::new())), None),
                    (msg(|m| m.set_clipboard(Clipboard::new())), None),
                    (
                        msg(|m| m.set_multi_clipboards(MultiClipboards::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_toggle_virtual_display(ToggleVirtualDisplay::new())),
                        Some("misc.toggle_virtual_display"),
                    ),
                    (
                        misc_msg(|m| m.set_change_resolution(Resolution::new())),
                        Some("misc.change_resolution"),
                    ),
                    (
                        misc_msg(|m| m.set_change_display_resolution(DisplayResolution::new())),
                        Some("misc.change_display_resolution"),
                    ),
                    (misc_msg(|m| m.set_refresh_video(true)), None),
                    (misc_msg(|m| m.set_refresh_video_display(0)), None),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default())
                        }),
                        None,
                    ),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default());
                            o.disable_audio = BoolOption::Yes.into();
                        }),
                        Some("misc.option"),
                    ),
                    (
                        msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                        Some("port_forward_channel"),
                    ),
                ],
            ),
            (
                AuthConnType::ViewCamera,
                vec![
                    (
                        misc_msg(|m| m.set_switch_display(SwitchDisplay::new())),
                        None,
                    ),
                    (misc_msg(|m| m.set_chat_message(ChatMessage::new())), None),
                    (
                        msg(|m| m.set_voice_call_request(VoiceCallRequest::new())),
                        None,
                    ),
                    (msg(|m| m.set_audio_frame(AudioFrame::new())), None),
                    (
                        option_msg(|o| o.image_quality = ImageQuality::Balanced.into()),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_toggle_privacy_mode(TogglePrivacyMode::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_toggle_virtual_display(ToggleVirtualDisplay::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_change_resolution(Resolution::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_change_display_resolution(DisplayResolution::new())),
                        None,
                    ),
                    (msg(|m| m.set_mouse_event(MouseEvent::new())), None),
                    (
                        msg(|m| m.set_pointer_device_event(PointerDeviceEvent::new())),
                        None,
                    ),
                    (msg(|m| m.set_key_event(KeyEvent::new())), None),
                    (misc_msg(|m| m.set_client_record_status(true)), None),
                    (
                        msg(|m| m.set_file_response(FileResponse::new())),
                        Some("file_response"),
                    ),
                    (
                        msg(|m| m.set_terminal_action(TerminalAction::new())),
                        Some("terminal_action"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                ],
            ),
            (
                AuthConnType::Remote,
                vec![
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        None,
                    ),
                    (msg(|m| m.set_terminal_action(TerminalAction::new())), None),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        None,
                    ),
                ],
            ),
            (
                AuthConnType::PortForward,
                vec![
                    (msg(|m| m.set_test_delay(TestDelay::new())), None),
                    (misc_msg(|m| m.set_close_reason("closed".to_owned())), None),
                    (
                        msg(|m| m.set_file_action(FileAction::new())),
                        Some("file_action"),
                    ),
                    (
                        msg(|m| m.set_terminal_action(TerminalAction::new())),
                        Some("terminal_action"),
                    ),
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        Some("screenshot_request"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                    (misc_msg(|m| m.set_refresh_video(true)), None),
                    (misc_msg(|m| m.set_refresh_video_display(0)), None),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default())
                        }),
                        None,
                    ),
                    (
                        msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                        None,
                    ),
                ],
            ),
        ];

        for (conn_type, messages) in cases {
            assert_scopes(conn_type, messages);
        }
    }

    #[test]
    fn session_scope_login_options_are_limited_to_authenticated_session_type() {
        let mut option = OptionMessage::new();
        option.image_quality = ImageQuality::Balanced.into();
        option.disable_audio = BoolOption::Yes.into();
        option.block_input = BoolOption::Yes.into();
        option.privacy_mode = BoolOption::Yes.into();

        let (scoped, violation) =
            Connection::scoped_login_option(AuthConnType::ViewCamera, &option);
        let scoped = scoped.unwrap();
        assert_eq!(violation, Some("login.option"));
        assert_eq!(
            scoped.image_quality.enum_value(),
            Ok(ImageQuality::Balanced)
        );
        assert_eq!(scoped.disable_audio.enum_value(), Ok(BoolOption::Yes));
        assert_eq!(scoped.block_input.enum_value(), Ok(BoolOption::NotSet));
        assert_eq!(scoped.privacy_mode.enum_value(), Ok(BoolOption::NotSet));

        let (scoped, violation) =
            Connection::scoped_login_option(AuthConnType::FileTransfer, &option);
        assert!(scoped.is_none());
        assert_eq!(violation, Some("login.option"));
    }

    #[test]
    fn session_scope_limited_render_noop_options_reject_mixed_fields() {
        for conn_type in [
            AuthConnType::FileTransfer,
            AuthConnType::Terminal,
            AuthConnType::PortForward,
        ] {
            let supported_decoding_only = option_msg(set_supported_decoding);
            assert_eq!(
                Connection::authorized_message_scope_violation(conn_type, &supported_decoding_only),
                None
            );

            let mixed_option = option_msg(|o| {
                set_supported_decoding(o);
                o.disable_audio = BoolOption::Yes.into();
            });
            assert_eq!(
                Connection::authorized_message_scope_violation(conn_type, &mixed_option),
                Some("misc.option")
            );
        }
    }

    #[test]
    fn session_scope_view_camera_options_keep_only_camera_fields() {
        let mut option = OptionMessage::new();
        option.image_quality = ImageQuality::Balanced.into();
        option.custom_image_quality = 80;
        option.custom_fps = 24;
        set_supported_decoding(&mut option);
        option.disable_audio = BoolOption::Yes.into();
        option.block_input = BoolOption::Yes.into();
        option.disable_clipboard = BoolOption::Yes.into();
        option.enable_file_transfer = BoolOption::Yes.into();
        option.terminal_persistent = BoolOption::Yes.into();

        let (scoped, violation) =
            Connection::scoped_login_option(AuthConnType::ViewCamera, &option);
        let scoped = scoped.unwrap();
        assert_eq!(violation, Some("login.option"));
        assert_eq!(
            scoped.image_quality.enum_value(),
            Ok(ImageQuality::Balanced)
        );
        assert_eq!(scoped.custom_image_quality, 80);
        assert_eq!(scoped.custom_fps, 24);
        assert!(scoped.supported_decoding.is_some());
        assert_eq!(scoped.disable_audio.enum_value(), Ok(BoolOption::Yes));
        assert_eq!(scoped.block_input.enum_value(), Ok(BoolOption::NotSet));
        assert_eq!(
            scoped.disable_clipboard.enum_value(),
            Ok(BoolOption::NotSet)
        );
        assert_eq!(
            scoped.enable_file_transfer.enum_value(),
            Ok(BoolOption::NotSet)
        );
        assert_eq!(
            scoped.terminal_persistent.enum_value(),
            Ok(BoolOption::NotSet)
        );
    }
    #[test]
    fn only_a_newer_remote_control_of_the_same_session_keeps_the_screen_unlocked() {
        let replaced_by = super::raii::AuthedConnID::is_newer_session_remote;

        let key = |session_id, peer: &str| SessionKey {
            peer_id: peer.to_owned(),
            name: "".to_owned(),
            session_id,
        };
        let conn = |conn_id, conn_type, session_key| AuthedConn {
            conn_id,
            conn_type,
            session_key,
            sender: mpsc::unbounded_channel().0,
        };
        let mine = key(7, "peer");
        let remote = AuthConnType::Remote;

        assert!(replaced_by(&conn(3, remote, mine.clone()), 2, &mine));
        // An older one, and itself: of connections ending at once only the last still locks.
        assert!(!replaced_by(&conn(1, remote, mine.clone()), 2, &mine));
        assert!(!replaced_by(&conn(2, remote, mine.clone()), 2, &mine));
        // A kind that keeps no screen in use.
        assert!(!replaced_by(
            &conn(3, AuthConnType::Terminal, mine.clone()),
            2,
            &mine
        ));
        // Another session of this peer, and another peer on the same session id: `SessionKey`
        // is all three fields, and either of those is someone else's screen to lock.
        assert!(!replaced_by(&conn(3, remote, key(8, "peer")), 2, &mine));
        assert!(!replaced_by(&conn(3, remote, key(7, "other")), 2, &mine));
    }
}
