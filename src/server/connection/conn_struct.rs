use super::*;

pub struct Connection {
    pub(super) inner: ConnInner,
    pub(super) display_idx: usize,
    pub(super) stream: super::super::Stream,
    pub(super) server: super::super::ServerPtrWeak,
    pub(super) hash: Hash,
    pub(super) read_jobs: Vec<fs::TransferJob>,
    pub(super) timer: crate::RustDeskInterval,
    pub(super) file_timer: crate::RustDeskInterval,
    pub(super) file_transfer: Option<(String, bool)>,
    pub(super) view_camera: bool,
    pub(super) terminal: bool,
    pub(super) port_forward_socket: Option<Framed<TcpStream, BytesCodec>>,
    pub(super) port_forward_mux: Option<super::super::port_forward_mux::PortForwardMux>,
    pub(super) port_forward_address: String,
    pub(super) tx_to_cm: mpsc::UnboundedSender<ipc::Data>,
    pub(super) authorized: bool,
    pub(super) require_2fa: Option<totp_rs::TOTP>,
    pub(super) awaiting_2fa: bool,
    pub(super) keyboard: bool,
    pub(super) clipboard: bool,
    pub(super) audio: bool,
    pub(super) file: bool,
    pub(super) restart: bool,
    pub(super) recording: bool,
    pub(super) block_input: bool,
    pub(super) privacy_mode: bool,
    pub(super) control_permissions: Option<ControlPermissions>,
    pub(super) last_test_delay: Option<Instant>,
    pub(super) network_delay: u32,
    pub(super) lock_after_session_end: bool,
    pub(super) show_remote_cursor: bool,
    // by peer
    pub(super) ip: String,
    // by peer
    pub(super) disable_keyboard: bool,
    // by peer
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) show_my_cursor: bool,
    // by peer
    pub(super) disable_clipboard: bool,
    // by peer
    pub(super) disable_audio: bool,
    // by peer
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub(super) enable_file_transfer: bool,
    // by peer
    pub(super) audio_sender: Option<MediaSender>,
    // audio by the remote peer/client
    pub(super) tx_input: std_mpsc::Sender<MessageInput>,
    // handle input messages
    pub(super) video_ack_required: bool,
    // Diagnostics only, gated by `RUSTDESK_QOS_VERBOSE`: how long the shared
    // write path blocked this second.  The video send is inline in the message
    // loop, so a slow write also delays the delay probe and its reply.
    pub(super) video_send_max_ms: u32,
    pub(super) video_send_sum_ms: u32,
    pub(super) video_send_count: u32,
    pub(super) server_audit_conn: String,
    pub(super) server_audit_file: String,
    pub(super) controlled_context: Option<ControlledContext>,
    pub(super) lr: LoginRequest,
    // Authentication retries may update credentials, but not the requested session scope.
    // A digest, so no peer-controlled strings are retained.
    pub(super) login_scope: Option<[u8; 32]>,
    pub(super) peer_argb: u32,
    pub(super) session_last_recv_time: Option<Arc<Mutex<Instant>>>,
    pub(super) chat_unanswered: bool,
    pub(super) file_transferred: bool,
    #[cfg(windows)]
    pub(super) portable: PortableState,
    pub(super) from_switch: bool,
    pub(super) voice_call_request_timestamp: Option<NonZeroI64>,
    pub(super) voice_calling: bool,
    pub(super) options_in_login: Option<OptionMessage>,
    #[cfg(not(any(target_os = "ios")))]
    pub(super) pressed_modifiers: HashSet<rdev::Key>,
    pub(super) closed: bool,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) start_cm_ipc_para: Option<StartCmIpcPara>,
    pub(super) auto_disconnect_timer: Option<(Instant, u64)>,
    pub(super) authed_conn_id: Option<self::raii::AuthedConnID>,
    pub(super) file_remove_log_control: FileRemoveLogControl,
    pub(super) last_supported_encoding: Option<SupportedEncoding>,
    pub(super) services_subed: bool,
    pub(super) delayed_read_dir: Option<(String, bool)>,
    #[cfg(target_os = "macos")]
    pub(super) retina: Retina,
    pub(super) follow_remote_cursor: bool,
    pub(super) follow_remote_window: bool,
    pub(super) multi_ui_session: bool,
    pub(super) tx_from_authed: mpsc::UnboundedSender<ipc::Data>,
    // For post requests that need to be sent sequentially.
    // eg. post_conn_audit
    pub(super) tx_post_seq: mpsc::UnboundedSender<(String, Value)>,
    pub(super) conn_audit_primary_auth: ConnAuditPrimaryAuth,
    pub(super) conn_audit_two_factor: ConnAuditTwoFactor,
    // Tracks read job IDs delegated to CM process.
    // When a read job is delegated to CM (via FS::ReadFile), the job id is added here.
    // Used to filter stale responses (FileBlockFromCM, FileReadDone, etc.) for
    // cancelled or unknown jobs.
    pub(super) cm_read_job_ids: HashSet<i32>,
    pub(super) terminal_service_id: String,
    pub(super) terminal_persistent: bool,
    // Used to avoid too many repeated scope violation warnings.
    pub(super) scope_violation_messages: HashSet<&'static str>,
    // The user token must be set when terminal is enabled.
    // 0 indicates SYSTEM user
    // other values indicate current user
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) terminal_user_token: Option<TerminalUserToken>,
    pub(super) terminal_generic_service: Option<Box<GenericService>>,
}
