//! Test-only construction of a `Connection` so the message handlers can be
//! driven without a rendezvous server, a CM process or a remote peer.

use super::*;
use hbb_common::tokio::net::TcpListener;

impl Connection {
    /// Build a connection over a loopback TCP pair. The returned stream is the
    /// controller's end: whatever the connection sends can be read from it.
    ///
    /// No CM IPC parameters are stored, so `try_start_cm_ipc` is a no-op and
    /// nothing is spawned. Audit posts and CM messages go to channels whose
    /// receivers are dropped.
    pub(super) async fn for_test(id: i32) -> (Self, super::super::Stream) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let client = TcpStream::connect(addr).await.expect("connect loopback");
        let (server_side, peer_addr) = listener.accept().await.expect("accept loopback");
        let stream = super::super::Stream::from(server_side, peer_addr);
        let controller = super::super::Stream::from(client, addr);

        let hash = Hash {
            salt: "test-salt".to_owned(),
            challenge: "test-challenge".to_owned(),
            ..Default::default()
        };
        let (tx_to_cm, _rx_to_cm) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx, _rx) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_video, _rx_video) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_input, _rx_input) = std_mpsc::channel();
        let (tx_from_authed, _rx_from_authed) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx_post_seq, _rx_post_seq) = mpsc::unbounded_channel();
        let control_permissions: Option<ControlPermissions> = None;
        let conn = Self {
            inner: ConnInner {
                id,
                tx: Some(tx),
                tx_video: Some(tx_video),
            },
            require_2fa: None,
            awaiting_2fa: false,
            display_idx: 0,
            stream,
            server: std::sync::Weak::new(),
            hash,
            read_jobs: Vec::new(),
            timer: crate::rustdesk_interval(time::interval(SEC30)),
            file_timer: crate::rustdesk_interval(time::interval(SEC30)),
            file_transfer: None,
            view_camera: false,
            terminal: false,
            port_forward_socket: None,
            port_forward_mux: None,
            port_forward_address: "".to_owned(),
            tx_to_cm,
            authorized: false,
            keyboard: Self::permission(keys::OPTION_ENABLE_KEYBOARD, &control_permissions),
            clipboard: Self::permission(keys::OPTION_ENABLE_CLIPBOARD, &control_permissions),
            audio: Self::permission(keys::OPTION_ENABLE_AUDIO, &control_permissions),
            file: Self::permission(keys::OPTION_ENABLE_FILE_TRANSFER, &control_permissions),
            restart: Self::permission(keys::OPTION_ENABLE_REMOTE_RESTART, &control_permissions),
            recording: Self::permission(keys::OPTION_ENABLE_RECORD_SESSION, &control_permissions),
            block_input: Self::permission(keys::OPTION_ENABLE_BLOCK_INPUT, &control_permissions),
            privacy_mode: Self::permission(keys::OPTION_ENABLE_PRIVACY_MODE, &control_permissions),
            control_permissions,
            last_test_delay: None,
            network_delay: 0,
            lock_after_session_end: false,
            show_remote_cursor: false,
            follow_remote_cursor: false,
            follow_remote_window: false,
            multi_ui_session: false,
            ip: peer_addr.ip().to_string(),
            disable_audio: false,
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
            enable_file_transfer: false,
            disable_clipboard: false,
            disable_keyboard: false,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            show_my_cursor: false,
            tx_input,
            video_ack_required: false,
            video_send_max_ms: 0,
            video_send_sum_ms: 0,
            video_send_count: 0,
            server_audit_conn: "".to_owned(),
            server_audit_file: "".to_owned(),
            controlled_context: None,
            lr: Default::default(),
            login_scope: None,
            peer_argb: 0u32,
            session_last_recv_time: None,
            chat_unanswered: false,
            file_transferred: false,
            #[cfg(windows)]
            portable: Default::default(),
            from_switch: false,
            audio_sender: None,
            voice_call_request_timestamp: None,
            voice_calling: false,
            options_in_login: None,
            #[cfg(not(any(target_os = "ios")))]
            pressed_modifiers: Default::default(),
            closed: false,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            start_cm_ipc_para: None,
            auto_disconnect_timer: None,
            authed_conn_id: None,
            file_remove_log_control: FileRemoveLogControl::new(id),
            last_supported_encoding: None,
            services_subed: false,
            delayed_read_dir: None,
            #[cfg(target_os = "macos")]
            retina: Retina::default(),
            tx_from_authed,
            tx_post_seq,
            cm_read_job_ids: HashSet::new(),
            terminal_service_id: "".to_owned(),
            terminal_persistent: false,
            scope_violation_messages: HashSet::new(),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            terminal_user_token: None,
            terminal_generic_service: None,
            conn_audit_primary_auth: ConnAuditPrimaryAuth::None,
            conn_audit_two_factor: ConnAuditTwoFactor::None,
        };
        (conn, controller)
    }

    /// The `LoginRequest.password` a controller would send for `password`:
    /// sha256(sha256(password + salt) + challenge).
    pub(super) fn hashed_login_password(&self, password: &str) -> Vec<u8> {
        let mut h1 = Sha256::new();
        h1.update(password.as_bytes());
        h1.update(self.hash.salt.as_bytes());
        let h1 = h1.finalize();
        let mut h2 = Sha256::new();
        h2.update(&h1[..]);
        h2.update(self.hash.challenge.as_bytes());
        h2.finalize().to_vec()
    }
}

/// Read the next protobuf message the connection sent to the controller.
pub(super) async fn next_message(controller: &mut super::super::Stream) -> Message {
    let bytes = timeout(3_000, controller.next())
        .await
        .expect("no message within 3 s")
        .expect("stream closed")
        .expect("stream error");
    Message::parse_from_bytes(&bytes).expect("parse message")
}
