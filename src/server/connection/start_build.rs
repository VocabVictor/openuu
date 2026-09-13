use super::*;

/// The receiving ends of the channels `Connection::build` wires into a connection.
pub(super) struct StartChannels {
    pub(super) rx_from_cm: mpsc::UnboundedReceiver<ipc::Data>,
    // holding tx_from_cm_holder to avoid cpu burning of rx_from_cm.recv when all sender closed
    pub(super) _tx_from_cm_holder: mpsc::UnboundedSender<ipc::Data>,
    #[cfg(target_os = "android")]
    pub(super) rx_to_cm: mpsc::UnboundedReceiver<ipc::Data>,
    #[cfg(target_os = "android")]
    pub(super) tx_from_cm: mpsc::UnboundedSender<ipc::Data>,
    pub(super) rx: mpsc::UnboundedReceiver<(Instant, Arc<Message>)>,
    pub(super) rx_video: mpsc::UnboundedReceiver<(Instant, Arc<Message>)>,
    pub(super) rx_input: std_mpsc::Receiver<MessageInput>,
    pub(super) rx_from_authed: mpsc::UnboundedReceiver<ipc::Data>,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) tx_cloned: mpsc::UnboundedSender<(Instant, Arc<Message>)>,
}

impl Connection {
    /// Create the connection state and its channels; nothing is sent yet.
    pub(super) fn build(
        stream: super::super::Stream,
        id: i32,
        server: super::super::ServerPtrWeak,
        control_permissions: Option<ControlPermissions>,
        controlled_context: Option<ControlledContext>,
    ) -> (Self, StartChannels) {
        let salt = Config::get_effective_permanent_password_salt();
        let hash = Hash {
            salt,
            challenge: Config::get_auto_password(6),
            ..Default::default()
        };
        let (tx_from_cm_holder, rx_from_cm) = mpsc::unbounded_channel::<ipc::Data>();
        // holding tx_from_cm_holder to avoid cpu burning of rx_from_cm.recv when all sender closed
        let tx_from_cm = tx_from_cm_holder.clone();
        let (tx_to_cm, rx_to_cm) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx, rx) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_video, rx_video) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_input, rx_input) = std_mpsc::channel();
        let (tx_from_authed, rx_from_authed) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx_post_seq, rx_post_seq) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            Self::post_seq_loop(rx_post_seq).await;
        });

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let tx_cloned = tx.clone();
        let conn = Self {
            inner: ConnInner {
                id,
                tx: Some(tx),
                tx_video: Some(tx_video),
            },
            require_2fa: crate::auth_2fa::get_2fa(None),
            awaiting_2fa: false,
            // Defer display enumeration until login succeeds. Monitor login replaces this
            // with the primary index returned with the refreshed display snapshot.
            display_idx: 0,
            stream: wire::Wire::Whole(stream),
            server,
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
            // to-do: make sure is the option correct here
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
            ip: "".to_owned(),
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
            video_send_bits: 0,
            video_blocked_ms: 0,
            server_audit_conn: "".to_owned(),
            server_audit_file: "".to_owned(),
            controlled_context,
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
            start_cm_ipc_para: Some(StartCmIpcPara {
                rx_to_cm,
                tx_from_cm,
            }),
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
        let channels = StartChannels {
            rx_from_cm,
            _tx_from_cm_holder: tx_from_cm_holder,
            #[cfg(target_os = "android")]
            rx_to_cm,
            #[cfg(target_os = "android")]
            tx_from_cm,
            rx,
            rx_video,
            rx_input,
            rx_from_authed,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            tx_cloned,
        };
        (conn, channels)
    }

    /// Tell the peer which permissions are off from the start.
    pub(super) async fn send_denied_permissions(&mut self) {
        #[cfg(target_os = "android")]
        self.send_permission(Permission::Keyboard, self.keyboard)
            .await;
        #[cfg(not(target_os = "android"))]
        if !self.keyboard {
            self.send_permission(Permission::Keyboard, false).await;
        }
        if !self.clipboard {
            self.send_permission(Permission::Clipboard, false).await;
        }
        if !self.audio {
            self.send_permission(Permission::Audio, false).await;
        }
        if !self.file {
            self.send_permission(Permission::File, false).await;
        }
        if !self.restart {
            self.send_permission(Permission::Restart, false).await;
        }
        if !self.recording {
            self.send_permission(Permission::Recording, false).await;
        }
        if !self.block_input {
            self.send_permission(Permission::BlockInput, false).await;
        }
        if !self.privacy_mode {
            self.send_permission(Permission::PrivacyMode, false).await;
        }
    }

    /// Leave the message loop: port forwarding, audit and cleanup.
    pub(super) async fn finish(mut self, rx_from_cm: &mut mpsc::UnboundedReceiver<ipc::Data>) {
        let id = self.inner.id();
        #[cfg(feature = "unix-file-copy-paste")]
        {
            self.try_empty_file_clipboard();
        }

        if let Some(video_privacy_conn_id) = privacy_mode::get_privacy_mode_conn_id() {
            if video_privacy_conn_id == id {
                let _ = Self::turn_off_privacy_to_msg(id, String::new());
            }
        }
        video_service::notify_video_frame_fetched_by_conn_id(id, None);
        if self.authorized {
            password::update_temporary_password();
        }
        if let Err(err) = self.try_port_forward_loop(rx_from_cm).await {
            self.on_close(&err.to_string(), false).await;
            raii::AuthedConnID::check_remove_session(self.inner.id(), self.session_key());
        }

        self.post_conn_audit(json!({
            "action": "close",
        }));
        if let Some(s) = self.server.upgrade() {
            let mut s = s.write().unwrap();
            s.remove_connection(&self.inner);
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            try_stop_record_cursor_pos();
        }
        self.on_close("End", true).await;
        log::info!("#{} connection loop exited", id);
    }
}
