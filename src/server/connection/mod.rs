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

impl Connection {
    async fn handle_read_job_init_result(
        &mut self,
        id: i32,
        _file_num: i32,
        _include_hidden: bool,
        result: Result<Vec<u8>, String>,
    ) {
        // Check if this response is still expected (not stale/cancelled)
        if !self.cm_read_job_ids.contains(&id) {
            log::warn!(
                "Received ReadJobInitResult for unknown or stale job id={}, ignoring",
                id
            );
            return;
        }

        match result {
            Err(error) => {
                self.cm_read_job_ids.remove(&id);
                self.send(fs::new_error(id, error, 0)).await;
            }
            Ok(dir_bytes) => {
                // Deserialize FileDirectory from protobuf bytes
                let dir = match FileDirectory::parse_from_bytes(&dir_bytes) {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("Failed to parse FileDirectory: {}", e);
                        self.cm_read_job_ids.remove(&id);
                        self.send(fs::new_error(id, "internal error".to_string(), 0))
                            .await;
                        return;
                    }
                };

                let path_str = dir.path.clone();
                let file_entries: Vec<FileEntry> = dir.entries.into();

                // Send file directory to client
                self.send(fs::new_dir(id, path_str.clone(), file_entries.clone()))
                    .await;

                // Post audit for file transfer
                self.post_file_audit(
                    FileAuditType::RemoteSend,
                    &path_str,
                    Self::get_files_for_audit(fs::JobType::Generic, file_entries),
                    json!({}),
                );

                // CM will handle the actual file reading and send blocks via IPC
                self.file_transferred = true;
            }
        }
    }

    async fn handle_file_block_from_cm(
        &mut self,
        id: i32,
        file_num: i32,
        data: bytes::Bytes,
        compressed: bool,
    ) {
        // Check if the job is still valid (not cancelled)
        if !self.cm_read_job_ids.contains(&id) {
            log::debug!(
                "Dropping file block for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward file block to client
        let mut block = FileTransferBlock::new();
        block.id = id;
        block.file_num = file_num;
        block.data = data.to_vec().into();
        block.compressed = compressed;

        let mut msg = Message::new();
        let mut fr = FileResponse::new();
        fr.set_block(block);
        msg.set_file_response(fr);
        self.send(msg).await;
    }

    async fn handle_file_read_done(&mut self, id: i32, file_num: i32) {
        // Drop stale completions for cancelled/unknown jobs
        if !self.cm_read_job_ids.remove(&id) {
            log::debug!(
                "Dropping FileReadDone for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward done message to client
        let mut done = FileTransferDone::new();
        done.id = id;
        done.file_num = file_num;

        let mut msg = Message::new();
        let mut fr = FileResponse::new();
        fr.set_done(done);
        msg.set_file_response(fr);
        self.send(msg).await;
    }

    async fn handle_file_read_error(&mut self, id: i32, file_num: i32, err: String) {
        // Drop stale errors for cancelled/unknown jobs
        if !self.cm_read_job_ids.remove(&id) {
            log::debug!(
                "Dropping FileReadError for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward error to client
        self.send(fs::new_error(id, err, file_num)).await;
    }

    async fn handle_file_digest_from_cm(
        &mut self,
        id: i32,
        file_num: i32,
        last_modified: u64,
        file_size: u64,
        is_resume: bool,
    ) {
        // Check if the job is still valid (not cancelled)
        if !self.cm_read_job_ids.contains(&id) {
            log::debug!(
                "Dropping digest for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward digest to client for overwrite detection
        let mut digest = FileTransferDigest::new();
        digest.id = id;
        digest.file_num = file_num;
        digest.last_modified = last_modified;
        digest.file_size = file_size;
        digest.is_upload = false; // Server sending to client
        digest.is_resume = is_resume;

        let mut msg = Message::new();
        let mut fr = FileResponse::new();
        fr.set_digest(digest);
        msg.set_file_response(fr);
        self.send(msg).await;
    }

    async fn process_new_read_job(&mut self, mut job: fs::TransferJob, path: String) {
        let files = job.files().to_owned();
        let job_type = job.r#type;
        self.send(fs::new_dir(job.id, path.clone(), files.clone()))
            .await;
        job.is_remote = true;
        job.conn_id = self.inner.id();
        self.read_jobs.push(job);
        self.file_timer = crate::rustdesk_interval(time::interval(MILLI1));
        let audit_path = path;
        self.post_file_audit(
            FileAuditType::RemoteSend,
            &audit_path,
            Self::get_files_for_audit(job_type, files),
            json!({}),
        );
    }

    async fn handle_all_files_result(
        &mut self,
        id: i32,
        path: String,
        result: Result<Vec<u8>, String>,
    ) {
        match result {
            Err(err) => {
                self.send(fs::new_error(id, err, -1)).await;
            }
            Ok(bytes) => {
                // Deserialize FileDirectory from protobuf bytes and send as FileResponse
                match FileDirectory::parse_from_bytes(&bytes) {
                    Ok(fd) => {
                        let mut msg = Message::new();
                        let mut fr = FileResponse::new();
                        fr.set_dir(fd);
                        msg.set_file_response(fr);
                        self.send(msg).await;
                    }
                    Err(e) => {
                        self.send(fs::new_error(
                            id,
                            format!("deserialize failed for {}: {}", path, e),
                            -1,
                        ))
                        .await;
                    }
                }
            }
        }
    }

    fn read_empty_dirs(&mut self, dir: &str, include_hidden: bool) {
        let dir = dir.to_string();
        self.send_fs(ipc::FS::ReadEmptyDirs {
            dir,
            include_hidden,
        });
    }

    fn read_dir(&mut self, dir: &str, include_hidden: bool) {
        let dir = dir.to_string();
        self.send_fs(ipc::FS::ReadDir {
            dir,
            include_hidden,
        });
    }

    /// Create a new read job and start processing it (Connection-side).
    ///
    /// This is a generic Connection-side read job creation helper used for:
    /// - Generic file transfers on non-Windows platforms
    ///
    /// On Windows, generic file reads are delegated to CM via `start_read_job()` in
    /// `src/ui_cm_interface.rs` for elevated access.
    ///
    /// Both Connection-side and CM-side implementations use `TransferJob::new_read()`
    /// with similar parameters. When modifying job creation logic, ensure both paths
    /// stay in sync.
    async fn create_and_start_read_job(
        &mut self,
        id: i32,
        job_type: fs::JobType,
        data_source: fs::DataSource,
        file_num: i32,
        include_hidden: bool,
        overwrite_detection: bool,
        path: String,
        check_file_limit: bool,
    ) {
        match fs::TransferJob::new_read(
            id,
            job_type,
            "".to_string(),
            data_source,
            file_num,
            include_hidden,
            false,
            overwrite_detection,
        ) {
            Err(err) => {
                self.send(fs::new_error(id, err, 0)).await;
            }
            Ok(job) => {
                if check_file_limit {
                    if let Err(msg) =
                        crate::ui_cm_interface::check_file_count_limit(job.files().len())
                    {
                        self.send(fs::new_error(id, msg, -1)).await;
                        return;
                    }
                }
                self.process_new_read_job(job, path).await;
            }
        }
    }

    #[inline]
    async fn send(&mut self, msg: Message) {
        allow_err!(self.stream.send(&msg).await);
    }

    pub fn alive_conns() -> Vec<i32> {
        ALIVE_CONNS.lock().unwrap().clone()
    }

    #[cfg(windows)]
    fn portable_check(&mut self) {
        if self.portable.is_installed || !self.is_remote() || !self.keyboard {
            return;
        }
        let running = portable_client::running();
        let show_elevation = !running;
        self.send_to_cm(ipc::Data::DataPortableService(
            ipc::DataPortableService::CmShowElevation(show_elevation),
        ));
        if self.authorized {
            let p = &mut self.portable;
            if Some(running) != p.last_running {
                p.last_running = Some(running);
                let mut misc = Misc::new();
                misc.set_portable_service_running(running);
                let mut msg = Message::new();
                msg.set_misc(misc);
                self.inner.send(msg.into());
            }
            let uac = crate::video_service::IS_UAC_RUNNING.lock().unwrap().clone();
            if p.last_uac != uac {
                p.last_uac = uac;
                if !uac || !running {
                    let mut misc = Misc::new();
                    misc.set_uac(uac);
                    let mut msg = Message::new();
                    msg.set_misc(misc);
                    self.inner.send(msg.into());
                }
            }
            let foreground_window_elevated = crate::video_service::IS_FOREGROUND_WINDOW_ELEVATED
                .lock()
                .unwrap()
                .clone();
            if p.last_foreground_window_elevated != foreground_window_elevated {
                p.last_foreground_window_elevated = foreground_window_elevated;
                if !foreground_window_elevated || !running {
                    let mut misc = Misc::new();
                    misc.set_foreground_window_elevated(foreground_window_elevated);
                    let mut msg = Message::new();
                    msg.set_misc(misc);
                    self.inner.send(msg.into());
                }
            }
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn release_pressed_modifiers(&mut self) {
        for modifier in self.pressed_modifiers.iter() {
            rdev::simulate(&rdev::EventType::KeyRelease(*modifier)).ok();
        }
        self.pressed_modifiers.clear();
    }

    fn get_auto_disconenct_timer() -> Option<(Instant, u64)> {
        if Config::get_option("allow-auto-disconnect") == "Y" {
            let mut minute: u64 = Config::get_option("auto-disconnect-timeout")
                .parse()
                .unwrap_or(10);
            if minute == 0 {
                minute = 10;
            }
            Some((Instant::now(), minute))
        } else {
            None
        }
    }

    fn update_auto_disconnect_timer(&mut self) {
        self.auto_disconnect_timer
            .as_mut()
            .map(|t| t.0 = Instant::now());
    }

    #[cfg(feature = "hwcodec")]
    fn update_supported_encoding(&mut self) {
        let Some(last) = &self.last_supported_encoding else {
            return;
        };
        let usable = scrap::codec::Encoder::usable_encoding();
        let Some(usable) = usable else {
            return;
        };
        if usable.vp8 != last.vp8
            || usable.av1 != last.av1
            || usable.h264 != last.h264
            || usable.h265 != last.h265
        {
            let mut misc: Misc = Misc::new();
            let supported_encoding = SupportedEncoding {
                vp8: usable.vp8,
                av1: usable.av1,
                h264: usable.h264,
                h265: usable.h265,
                ..last.clone()
            };
            log::info!("update supported encoding: {:?}", supported_encoding);
            self.last_supported_encoding = Some(supported_encoding.clone());
            misc.set_supported_encoding(supported_encoding);
            let mut msg = Message::new();
            msg.set_misc(misc);
            self.inner.send(msg.into());
        };
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    async fn handle_cursor_switch_display(&mut self, pos: CursorPosition) {
        if self.multi_ui_session {
            return;
        }
        let displays = super::display_service::get_sync_displays();
        let d_index = displays.iter().position(|d| {
            let scale = d.scale;
            pos.x >= d.x
                && pos.y >= d.y
                && (pos.x - d.x) as f64 * scale < d.width as f64
                && (pos.y - d.y) as f64 * scale < d.height as f64
        });
        if let Some(d_index) = d_index {
            if self.display_idx != d_index {
                let mut misc = Misc::new();
                misc.set_follow_current_display(d_index as i32);
                let mut msg_out = Message::new();
                msg_out.set_misc(misc);
                self.send(msg_out).await;
            }
        }
    }

    #[inline]
    fn session_key(&self) -> SessionKey {
        SessionKey {
            peer_id: self.lr.my_id.clone(),
            name: self.lr.my_name.clone(),
            session_id: self.lr.session_id,
        }
    }

    fn is_authed_remote_conn(&self) -> bool {
        if let Some(id) = self.authed_conn_id.as_ref() {
            return id.conn_type() == AuthConnType::Remote;
        }
        false
    }

    fn is_authed_view_camera_conn(&self) -> bool {
        if let Some(id) = self.authed_conn_id.as_ref() {
            return id.conn_type() == AuthConnType::ViewCamera;
        }
        false
    }

    fn should_handle_render_broadcast_message(&self) -> bool {
        matches!(
            self.authed_conn_type(),
            Some(AuthConnType::Remote | AuthConnType::ViewCamera)
        )
    }

    fn should_handle_text_clipboard_message(&self) -> bool {
        matches!(self.authed_conn_type(), Some(AuthConnType::Remote))
    }

    fn scoped_update_option_message(&self, option: &OptionMessage) -> Option<OptionMessage> {
        match self.authed_conn_type() {
            Some(AuthConnType::ViewCamera) => Self::scoped_view_camera_option(option).0,
            Some(AuthConnType::Terminal) => Self::scoped_terminal_login_option(option).0,
            Some(AuthConnType::Remote | AuthConnType::FileTransfer | AuthConnType::PortForward)
            | None => None,
        }
    }

    fn authed_conn_type(&self) -> Option<AuthConnType> {
        self.authed_conn_id.as_ref().map(|id| id.conn_type())
    }

    async fn handle_authorized_scope_violation(&mut self, message: &'static str) -> bool {
        let conn_type = self
            .authed_conn_type()
            .map(AuthConnType::as_str)
            .unwrap_or("unknown");
        let is_first = self.scope_violation_messages.insert(message);
        if is_first {
            log::warn!(
                "Received out-of-scope message in {} session: {}",
                conn_type,
                message
            );
        } else {
            log::debug!(
                "Received repeated out-of-scope message in {} session: {}",
                conn_type,
                message
            );
        }
        if is_first && Config::get_bool_option(keys::OPTION_ALLOW_SCOPE_VIOLATION_ALARM) {
            self.post_session_scope_violation_alarm(message);
        }
        if Config::get_bool_option(keys::OPTION_ALLOW_SCOPE_VIOLATION_CLOSE) {
            self.send_close_reason_no_retry("Connection not allowed")
                .await;
            self.on_close("Session scope violation", true).await;
            return false;
        }
        true
    }

    fn authorized_scope_violation(&self, msg: &Message) -> Option<&'static str> {
        let Some(conn_type) = self.authed_conn_type() else {
            return (!Self::is_connection_housekeeping_message(msg)).then_some("session.auth_type");
        };
        Self::authorized_message_scope_violation(conn_type, msg)
    }

    async fn update_scoped_login_options(&mut self) {
        let Some(option) = self.options_in_login.take() else {
            return;
        };
        let Some(conn_type) = self.authed_conn_type() else {
            // Unreachable, but just in case, we drop the options if the connection type is unknown.
            log::warn!(
                "Dropping scoped login options because authorized connection type is unknown"
            );
            return;
        };
        let (scoped, violation) = Self::scoped_login_option(conn_type, &option);
        if let Some(message) = violation {
            log::debug!(
                "Filtering {} session login options outside scope: {}",
                conn_type.as_str(),
                message
            );
        }
        if let Some(option) = scoped {
            self.update_options(&option).await;
        }
    }

    fn scoped_login_option(
        conn_type: AuthConnType,
        option: &OptionMessage,
    ) -> (Option<OptionMessage>, Option<&'static str>) {
        match conn_type {
            AuthConnType::Remote => (Some(option.clone()), None),
            AuthConnType::ViewCamera => Self::scoped_view_camera_option(option),
            AuthConnType::Terminal => Self::scoped_terminal_login_option(option),
            AuthConnType::FileTransfer | AuthConnType::PortForward => {
                let violation = Self::option_has_any_field(option).then_some("login.option");
                (None, violation)
            }
        }
    }

    fn scoped_terminal_login_option(
        option: &OptionMessage,
    ) -> (Option<OptionMessage>, Option<&'static str>) {
        let mut scoped = OptionMessage::new();
        let mut violation = false;
        match option.terminal_persistent.enum_value() {
            Ok(value) => scoped.terminal_persistent = value.into(),
            Err(_) => violation = true,
        }
        if Self::option_has_non_terminal_login_field(option) {
            violation = true;
        }
        let scoped = Self::option_has_any_field(&scoped).then_some(scoped);
        (scoped, violation.then_some("login.option"))
    }

    fn authorized_message_scope_violation(
        conn_type: AuthConnType,
        msg: &Message,
    ) -> Option<&'static str> {
        if Self::is_connection_housekeeping_message(msg) {
            return None;
        }
        // Legacy clients can broadcast render-refresh messages to all opened sessions.
        // Clipboard messages may also be broadcast to FileTransfer/Terminal sessions while
        // the client still considers text clipboard sync required, and handlers ignore them.
        let noop_compat = match conn_type {
            AuthConnType::FileTransfer | AuthConnType::Terminal => {
                Self::is_render_broadcast_noop_compat_message(msg)
                    || Self::is_text_clipboard_noop_compat_message(msg)
            }
            AuthConnType::PortForward => Self::is_render_broadcast_noop_compat_message(msg),
            AuthConnType::ViewCamera => Self::is_text_clipboard_noop_compat_message(msg),
            _ => false,
        };
        if noop_compat {
            return None;
        }
        let allowed = match conn_type {
            AuthConnType::Remote => true,
            AuthConnType::FileTransfer => Self::is_file_transfer_scoped_message(msg),
            AuthConnType::PortForward => Self::is_port_forward_scoped_message(msg),
            AuthConnType::ViewCamera => Self::is_view_camera_scoped_message(msg),
            AuthConnType::Terminal => Self::is_terminal_scoped_message(msg),
        };
        (!allowed).then(|| Self::message_family(msg))
    }

    fn is_render_broadcast_noop_compat_message(msg: &Message) -> bool {
        let Some(message::Union::Misc(misc)) = msg.union.as_ref() else {
            return false;
        };
        match misc.union.as_ref() {
            Some(misc::Union::RefreshVideo(_)) | Some(misc::Union::RefreshVideoDisplay(_)) => true,
            Some(misc::Union::Option(option)) => Self::is_supported_decoding_only_option(option),
            _ => false,
        }
    }

    fn is_text_clipboard_noop_compat_message(msg: &Message) -> bool {
        matches!(
            msg.union.as_ref(),
            Some(message::Union::Clipboard(_)) | Some(message::Union::MultiClipboards(_))
        )
    }

    fn is_supported_decoding_only_option(option: &OptionMessage) -> bool {
        option.supported_decoding.is_some()
            && option.image_quality.enum_value() == Ok(ImageQuality::NotSet)
            && option.custom_image_quality == 0
            && option.custom_fps == 0
            && Self::is_bool_option_not_set(option.lock_after_session_end)
            && Self::is_bool_option_not_set(option.show_remote_cursor)
            && Self::is_bool_option_not_set(option.privacy_mode)
            && Self::is_bool_option_not_set(option.block_input)
            && Self::is_bool_option_not_set(option.disable_audio)
            && Self::is_bool_option_not_set(option.disable_clipboard)
            && Self::is_bool_option_not_set(option.enable_file_transfer)
            && Self::is_bool_option_not_set(option.disable_keyboard)
            && Self::is_bool_option_not_set(option.follow_remote_cursor)
            && Self::is_bool_option_not_set(option.follow_remote_window)
            && Self::is_bool_option_not_set(option.disable_camera)
            && Self::is_bool_option_not_set(option.terminal_persistent)
            && Self::is_bool_option_not_set(option.show_my_cursor)
    }

    fn is_connection_housekeeping_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::LoginRequest(_)) => true,
            Some(message::Union::TestDelay(_)) => true,
            Some(message::Union::Misc(misc)) => {
                matches!(misc.union.as_ref(), Some(misc::Union::CloseReason(_)))
            }
            _ => false,
        }
    }

    fn is_file_transfer_scoped_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::FileAction(_)) | Some(message::Union::FileResponse(_)) => true,
            Some(message::Union::Misc(misc)) => Self::is_file_transfer_scoped_misc(misc),
            _ => false,
        }
    }

    fn is_file_transfer_scoped_misc(misc: &Misc) -> bool {
        #[cfg(windows)]
        if matches!(misc.union.as_ref(), Some(misc::Union::SelectedSid(_))) {
            return true;
        }
        #[cfg(not(windows))]
        let _ = misc;
        false
    }

    fn is_port_forward_scoped_message(msg: &Message) -> bool {
        matches!(
            msg.union.as_ref(),
            Some(message::Union::PortForwardChannel(_))
        )
    }

    fn is_terminal_scoped_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::TerminalAction(_)) => true,
            Some(message::Union::Misc(misc)) => Self::is_terminal_scoped_misc(misc),
            _ => false,
        }
    }

    fn is_terminal_scoped_misc(misc: &Misc) -> bool {
        match misc.union.as_ref() {
            Some(misc::Union::ChatMessage(_)) => true,
            Some(misc::Union::Option(option)) => Self::is_terminal_scoped_option(option),
            _ => false,
        }
    }

    fn is_terminal_scoped_option(option: &OptionMessage) -> bool {
        Self::scoped_terminal_login_option(option).1.is_none()
    }

    fn is_view_camera_scoped_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::ScreenshotRequest(_)) => true,
            Some(message::Union::Misc(misc)) => Self::is_view_camera_scoped_misc(misc),
            // Legacy clients may send auto-login input during view-camera connect.
            // The handlers intentionally ignore these messages for view-camera sessions.
            Some(message::Union::MouseEvent(_))
            | Some(message::Union::PointerDeviceEvent(_))
            | Some(message::Union::KeyEvent(_)) => true,
            Some(message::Union::AudioFrame(_))
            | Some(message::Union::VoiceCallRequest(_))
            | Some(message::Union::VoiceCallResponse(_)) => true,
            _ => false,
        }
    }

    fn is_view_camera_scoped_misc(misc: &Misc) -> bool {
        match misc.union.as_ref() {
            Some(misc::Union::SwitchDisplay(_))
            | Some(misc::Union::CaptureDisplays(_))
            | Some(misc::Union::RefreshVideo(_))
            | Some(misc::Union::RefreshVideoDisplay(_))
            | Some(misc::Union::VideoReceived(_))
            | Some(misc::Union::ChatMessage(_))
            | Some(misc::Union::AudioFormat(_))
            | Some(misc::Union::ClientRecordStatus(_))
            // Though these messages are not expected in normal view-camera sessions,
            // keep them allowed to avoid breaking existing clients that may send them.
            | Some(misc::Union::MessageQuery(_))
            | Some(misc::Union::TogglePrivacyMode(_))
            | Some(misc::Union::ToggleVirtualDisplay(_))
            | Some(misc::Union::ChangeResolution(_))
            | Some(misc::Union::ChangeDisplayResolution(_)) => true,
            Some(misc::Union::Option(option)) => Self::is_view_camera_scoped_option(option),
            #[cfg(windows)]
            Some(misc::Union::SelectedSid(_)) => true,
            _ => false,
        }
    }

    fn is_view_camera_scoped_option(option: &OptionMessage) -> bool {
        Self::scoped_view_camera_option(option).1.is_none()
    }

    // Keep these OptionMessage field lists in sync with message.proto and update_options().
    // New fields must be classified here before limited session types can receive them.
    fn scoped_view_camera_option(
        option: &OptionMessage,
    ) -> (Option<OptionMessage>, Option<&'static str>) {
        let mut scoped = OptionMessage::new();
        let mut violation = false;
        if option.image_quality.enum_value().is_ok() {
            scoped.image_quality = option.image_quality;
        }
        if option.custom_image_quality >= 0 {
            scoped.custom_image_quality = option.custom_image_quality;
        }
        if option.custom_fps >= 0 {
            scoped.custom_fps = option.custom_fps;
        }
        scoped.supported_decoding = option.supported_decoding.clone();
        if let Ok(value) = option.disable_audio.enum_value() {
            scoped.disable_audio = value.into();
        }
        if Self::option_has_non_view_camera_login_field(option) {
            violation = true;
        }
        let scoped = Self::option_has_any_field(&scoped).then_some(scoped);
        (scoped, violation.then_some("login.option"))
    }

    fn option_has_non_view_camera_login_field(option: &OptionMessage) -> bool {
        !(Self::is_bool_option_not_set(option.lock_after_session_end)
            && Self::is_bool_option_not_set(option.show_remote_cursor)
            && Self::is_bool_option_not_set(option.privacy_mode)
            && Self::is_bool_option_not_set(option.block_input)
            && Self::is_bool_option_not_set(option.disable_clipboard)
            && Self::is_bool_option_not_set(option.enable_file_transfer)
            && Self::is_bool_option_not_set(option.disable_keyboard)
            && Self::is_bool_option_not_set(option.follow_remote_cursor)
            && Self::is_bool_option_not_set(option.follow_remote_window)
            && Self::is_bool_option_not_set(option.disable_camera)
            && Self::is_bool_option_not_set(option.terminal_persistent)
            && Self::is_bool_option_not_set(option.show_my_cursor))
    }

    fn option_has_non_terminal_login_field(option: &OptionMessage) -> bool {
        option.image_quality.enum_value() != Ok(ImageQuality::NotSet)
            || option.custom_image_quality != 0
            || option.custom_fps != 0
            || option.supported_decoding.is_some()
            || !Self::is_bool_option_not_set(option.lock_after_session_end)
            || !Self::is_bool_option_not_set(option.show_remote_cursor)
            || !Self::is_bool_option_not_set(option.privacy_mode)
            || !Self::is_bool_option_not_set(option.block_input)
            || !Self::is_bool_option_not_set(option.disable_audio)
            || !Self::is_bool_option_not_set(option.disable_clipboard)
            || !Self::is_bool_option_not_set(option.enable_file_transfer)
            || !Self::is_bool_option_not_set(option.disable_keyboard)
            || !Self::is_bool_option_not_set(option.follow_remote_cursor)
            || !Self::is_bool_option_not_set(option.follow_remote_window)
            || !Self::is_bool_option_not_set(option.disable_camera)
            || !Self::is_bool_option_not_set(option.show_my_cursor)
    }

    fn option_has_any_field(option: &OptionMessage) -> bool {
        Self::option_has_non_terminal_login_field(option)
            || !Self::is_bool_option_not_set(option.terminal_persistent)
    }

    fn is_bool_option_not_set(option: hbb_common::protobuf::EnumOrUnknown<BoolOption>) -> bool {
        option.enum_value() == Ok(BoolOption::NotSet)
    }

    fn message_family(msg: &Message) -> &'static str {
        match msg.union.as_ref() {
            Some(message::Union::MouseEvent(_)) => "mouse_event",
            Some(message::Union::AudioFrame(_)) => "audio_frame",
            Some(message::Union::PointerDeviceEvent(_)) => "pointer_device_event",
            Some(message::Union::KeyEvent(_)) => "key_event",
            Some(message::Union::Clipboard(_)) => "clipboard",
            Some(message::Union::FileAction(_)) => "file_action",
            Some(message::Union::FileResponse(_)) => "file_response",
            Some(message::Union::VoiceCallRequest(_)) => "voice_call_request",
            Some(message::Union::VoiceCallResponse(_)) => "voice_call_response",
            Some(message::Union::MultiClipboards(_)) => "multi_clipboards",
            Some(message::Union::ScreenshotRequest(_)) => "screenshot_request",
            Some(message::Union::ScreenshotResponse(_)) => "screenshot_response",
            Some(message::Union::TerminalAction(_)) => "terminal_action",
            Some(message::Union::TerminalResponse(_)) => "terminal_response",
            Some(message::Union::PortForwardChannel(_)) => "port_forward_channel",
            Some(message::Union::Misc(misc)) => Self::misc_message_family(misc),
            Some(_) => "message.other",
            None => "empty",
        }
    }

    fn misc_message_family(misc: &Misc) -> &'static str {
        match misc.union.as_ref() {
            Some(misc::Union::ChatMessage(_)) => "misc.chat_message",
            Some(misc::Union::SwitchDisplay(_)) => "misc.switch_display",
            Some(misc::Union::Option(_)) => "misc.option",
            Some(misc::Union::AudioFormat(_)) => "misc.audio_format",
            Some(misc::Union::CaptureDisplays(_)) => "misc.capture_displays",
            Some(misc::Union::ClientRecordStatus(_)) => "misc.client_record_status",
            Some(misc::Union::TogglePrivacyMode(_)) => "misc.toggle_privacy_mode",
            Some(misc::Union::ToggleVirtualDisplay(_)) => "misc.toggle_virtual_display",
            Some(misc::Union::SelectedSid(_)) => "misc.selected_sid",
            Some(misc::Union::ChangeResolution(_)) => "misc.change_resolution",
            Some(misc::Union::ChangeDisplayResolution(_)) => "misc.change_display_resolution",
            Some(misc::Union::MessageQuery(_)) => "misc.message_query",
            Some(misc::Union::FollowCurrentDisplay(_)) => "misc.follow_current_display",
            Some(misc::Union::SwitchSidesRequest(_)) => "misc.switch_sides_request",
            Some(_) => "misc.other",
            None => "misc.empty",
        }
    }

    #[cfg(feature = "unix-file-copy-paste")]
    async fn handle_file_clip(&mut self, clip: clipboard::ClipboardFile) {
        let is_stopping_allowed = clip.is_stopping_allowed();
        let file_transfer_enabled = self.file_transfer_enabled();
        let stop = is_stopping_allowed && !file_transfer_enabled;
        log::debug!(
            "Process clipboard message from clip, stop: {}, is_stopping_allowed: {}, file_transfer_enabled: {}",
            stop, is_stopping_allowed, file_transfer_enabled);
        if !stop {
            use base::config::keys::OPTION_ONE_WAY_FILE_TRANSFER;
            // Note: Code will not reach here if `crate::get_builtin_option(OPTION_ONE_WAY_FILE_TRANSFER) == "Y"` is true.
            // Because `file-clipboard` service will not be subscribed.
            // But we still check it here to keep the same logic to windows version in `ui_cm_interface.rs`.
            if clip.is_beginning_message()
                && crate::get_builtin_option(OPTION_ONE_WAY_FILE_TRANSFER) == "Y"
            {
                // If one way file transfer is enabled, don't send clipboard file to client
            } else {
                // Maybe we should end the connection, because copy&paste files causes everything to wait.
                allow_err!(
                    self.stream
                        .send(&crate::clipboard_file::clip_2_msg(clip))
                        .await
                );
            }
        }
    }

    #[inline]
    #[cfg(feature = "unix-file-copy-paste")]
    fn try_empty_file_clipboard(&mut self) {
        try_empty_clipboard_files(ClipboardSide::Host, self.inner.id());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    async fn update_terminal_persistence(&mut self, persistent: bool) {
        self.terminal_persistent = persistent;
        terminal_service::set_persistent(&self.terminal_service_id, persistent).ok();
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    async fn init_terminal_service(&mut self) {
        debug_assert!(self.terminal_user_token.is_some());
        let Some(user_token) = self.terminal_user_token.clone() else {
            // unreachable, but keep it for safety
            log::error!("Terminal user token is not set.");
            return;
        };
        if self.terminal_service_id.is_empty() {
            self.terminal_service_id = terminal_service::generate_service_id();
        }
        let s = Box::new(terminal_service::new(
            self.terminal_service_id.clone(),
            self.terminal_persistent,
            user_token.to_terminal_service_token(),
        ));
        s.on_subscribe(self.inner.clone());
        self.terminal_generic_service = Some(s);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    async fn handle_terminal_action(&mut self, action: TerminalAction) -> ResultType<()> {
        debug_assert!(self.terminal_user_token.is_some());
        let Some(user_token) = self.terminal_user_token.clone() else {
            // unreacheable, but keep it for safety
            bail!("Terminal user token is not set.");
        };
        let mut proxy = terminal_service::TerminalServiceProxy::new(
            self.terminal_service_id.clone(),
            Some(self.terminal_persistent),
            user_token.to_terminal_service_token(),
        );

        match proxy.handle_action(&action) {
            Ok(Some(response)) => {
                let mut msg_out = Message::new();
                msg_out.set_terminal_response(response);
                self.send(msg_out).await;
            }
            Ok(None) => {
                // No response needed
            }
            Err(err) => {
                let mut response = TerminalResponse::new();
                let mut error = TerminalError::new();
                error.message = format!("Failed to handle action: {}", err);
                response.set_error(error);
                let mut msg_out = Message::new();
                msg_out.set_terminal_response(response);
                self.send(msg_out).await;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn insert_switch_sides_uuid(id: String, uuid: uuid::Uuid) {
    SWITCH_SIDES_UUID
        .lock()
        .unwrap()
        .insert(id, (tokio::time::Instant::now(), uuid));
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn insert_pending_switch_sides_uuid(id: String, uuid: uuid::Uuid) -> bool {
    let mut uuids = PENDING_SWITCH_SIDES_UUID.lock().unwrap();
    uuids.retain(|_, (instant, _, _)| instant.elapsed() < SWITCH_SIDES_UUID_TTL);
    if uuids.get(&id).map(|(_, stored_uuid, _)| stored_uuid) == Some(&uuid) {
        return false;
    }
    uuids.insert(id, (tokio::time::Instant::now(), uuid, false));
    true
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn has_pending_switch_sides_uuid(id: &str, uuid: &uuid::Uuid) -> bool {
    let mut uuids = PENDING_SWITCH_SIDES_UUID.lock().unwrap();
    uuids.retain(|_, (instant, _, _)| instant.elapsed() < SWITCH_SIDES_UUID_TTL);
    uuids
        .get(id)
        .map(|(_, stored_uuid, claimed)| stored_uuid == uuid && !*claimed)
        == Some(true)
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn claim_pending_switch_sides_uuid(id: &str, uuid: &uuid::Uuid) -> bool {
    let mut uuids = PENDING_SWITCH_SIDES_UUID.lock().unwrap();
    uuids.retain(|_, (instant, _, _)| instant.elapsed() < SWITCH_SIDES_UUID_TTL);
    // Keep claimed entries until expiry so replaying a request cannot launch another connection.
    if let Some((_, stored_uuid, claimed)) = uuids.get_mut(id) {
        if stored_uuid == uuid && !*claimed {
            *claimed = true;
            return true;
        }
    }
    false
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
// IPC bootstrap summary:
// - Start CM when missing, then bridge bidirectional messages between this task and CM IPC.
async fn start_ipc(
    mut rx_to_cm: mpsc::UnboundedReceiver<ipc::Data>,
    tx_from_cm: mpsc::UnboundedSender<ipc::Data>,
) -> ResultType<()> {
    use hbb_common::anyhow::anyhow;

    loop {
        if !crate::platform::is_prelogin() {
            break;
        }
        sleep(1.).await;
    }
    let mut stream = None;
    if let Ok(s) = crate::ipc::connect(1000, "_cm").await {
        stream = Some(s);
    }
    if stream.is_none() {
        let args = vec!["--cm"];
        let run_done;
        if crate::platform::is_root() {
            let mut res = Ok(None);
            for _ in 0..10 {
                #[cfg(not(any(target_os = "linux")))]
                {
                    log::debug!("Start cm");
                    res = crate::platform::run_as_user(args.clone());
                }
                #[cfg(target_os = "linux")]
                {
                    log::debug!("Start cm");
                    res = crate::platform::run_as_user(args.clone(), None, None::<(&str, &str)>);
                }
                if res.is_ok() {
                    break;
                }
                log::error!("Failed to run cm: {res:?}");
                sleep(1.).await;
            }
            if let Some(task) = res? {
                super::CHILD_PROCESS.lock().unwrap().push(task);
            }
            run_done = true;
        } else {
            run_done = false;
        }
        if !run_done {
            log::debug!("Start cm");
            super::CHILD_PROCESS
                .lock()
                .unwrap()
                .push(crate::run_me(args)?);
        }
        for _ in 0..20 {
            sleep(0.3).await;
            if let Ok(s) = crate::ipc::connect(1000, "_cm").await {
                stream = Some(s);
                break;
            }
        }
    }
    if stream.is_none() {
        bail!("Failed to connect to connection manager");
    }

    let mut stream = stream.ok_or(anyhow!("none stream"))?;
    loop {
        tokio::select! {
            res = stream.next() => {
                match res {
                    Err(err) => {
                        return Err(err.into());
                    }
                    Ok(Some(data)) => {
                        match data {
                            ipc::Data::ClickTime(_)=> {
                                let ct = CLICK_TIME.load(Ordering::SeqCst);
                                let data = ipc::Data::ClickTime(ct);
                                stream.send(&data).await?;
                            }
                            // FileBlockFromCM: data is always sent separately via send_raw.
                            // The data field has #[serde(skip)], so it's empty after deserialization.
                            // Read the raw data bytes following this message.
                            //
                            // Note: Empty data (for empty files) is correctly handled. BytesCodec with
                            // raw=false adds a length prefix, so next_raw() returns empty BytesMut for
                            // zero-length frames. This mirrors the WriteBlock pattern below.
                            ipc::Data::FileBlockFromCM { id, file_num, data: _, compressed, conn_id } => {
                                let raw_data = stream.next_raw().await?;
                                tx_from_cm.send(ipc::Data::FileBlockFromCM {
                                    id,
                                    file_num,
                                    data: raw_data.into(),
                                    compressed,
                                    conn_id,
                                })?;
                            }
                            _ => {
                                tx_from_cm.send(data)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            res = rx_to_cm.recv() => {
                match res {
                    Some(data) => {
                        if let Data::FS(ipc::FS::WriteBlock{id,
                            file_num,
                            data,
                            compressed}) = data {
                                stream.send(&Data::FS(ipc::FS::WriteBlock{id, file_num, data: Bytes::new(), compressed})).await?;
                                stream.send_raw(data).await?;
                        } else {
                            stream.send(&data).await?;
                        }
                    }
                    None => {
                        bail!("expected");
                    }
                }
            }
        }
    }
}

// in case screen is sleep and blank, here to activate it
fn try_activate_screen() {
    #[cfg(windows)]
    std::thread::spawn(|| {
        mouse_move_relative(-6, -6);
        std::thread::sleep(std::time::Duration::from_millis(30));
        mouse_move_relative(6, 6);
    });
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
