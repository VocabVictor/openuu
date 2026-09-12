use super::*;

impl Connection {
    pub async fn start(
        addr: SocketAddr,
        stream: super::super::Stream,
        id: i32,
        server: super::super::ServerPtrWeak,
        meta: super::super::ConnectionMeta,
    ) {
        let super::super::ConnectionMeta {
            control_permissions,
            controlled_context,
        } = meta;
        // Android is not supported yet, so we always set control_permissions to None.
        #[cfg(target_os = "android")]
        let control_permissions = None;
        let _raii_id = raii::ConnectionID::new(id);
        let _raii_control_permissions_id =
            raii::ControlPermissionsID::new(id, &control_permissions);
        let salt = Config::get_effective_permanent_password_salt();
        let hash = Hash {
            salt,
            challenge: Config::get_auto_password(6),
            ..Default::default()
        };
        let (tx_from_cm_holder, mut rx_from_cm) = mpsc::unbounded_channel::<ipc::Data>();
        // holding tx_from_cm_holder to avoid cpu burning of rx_from_cm.recv when all sender closed
        let tx_from_cm = tx_from_cm_holder.clone();
        let (tx_to_cm, rx_to_cm) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx, mut rx) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_video, mut rx_video) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_input, _rx_input) = std_mpsc::channel();
        let (tx_from_authed, mut rx_from_authed) = mpsc::unbounded_channel::<ipc::Data>();
        let mut hbbs_rx = crate::hbbs_http::sync::signal_receiver();
        let (tx_post_seq, rx_post_seq) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            Self::post_seq_loop(rx_post_seq).await;
        });

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let tx_cloned = tx.clone();
        let mut conn = Self {
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
            stream,
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
        let addr = hbb_common::try_into_v4(addr);
        if !conn.on_open(addr).await {
            conn.closed = true;
            // sleep to ensure msg got received.
            sleep(1.).await;
            return;
        }
        #[cfg(target_os = "android")]
        start_channel(rx_to_cm, tx_from_cm);
        #[cfg(target_os = "android")]
        conn.send_permission(Permission::Keyboard, conn.keyboard)
            .await;
        #[cfg(not(target_os = "android"))]
        if !conn.keyboard {
            conn.send_permission(Permission::Keyboard, false).await;
        }
        if !conn.clipboard {
            conn.send_permission(Permission::Clipboard, false).await;
        }
        if !conn.audio {
            conn.send_permission(Permission::Audio, false).await;
        }
        if !conn.file {
            conn.send_permission(Permission::File, false).await;
        }
        if !conn.restart {
            conn.send_permission(Permission::Restart, false).await;
        }
        if !conn.recording {
            conn.send_permission(Permission::Recording, false).await;
        }
        if !conn.block_input {
            conn.send_permission(Permission::BlockInput, false).await;
        }
        if !conn.privacy_mode {
            conn.send_permission(Permission::PrivacyMode, false).await;
        }
        let mut test_delay_timer =
            crate::rustdesk_interval(time::interval_at(Instant::now(), TEST_DELAY_TIMEOUT));
        let mut last_recv_time = Instant::now();

        // The connection type is not known until the login request arrives;
        // `on_message` picks the type-specific timeout then.
        conn.stream.set_send_timeout(SEND_TIMEOUT_VIDEO);

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        std::thread::spawn(move || Self::handle_input(_rx_input, tx_cloned));
        let mut second_timer = crate::rustdesk_interval(time::interval(Duration::from_secs(1)));

        #[cfg(feature = "unix-file-copy-paste")]
        let rx_clip_holder;
        let mut rx_clip;
        let _tx_clip: mpsc::UnboundedSender<i32>;
        #[cfg(feature = "unix-file-copy-paste")]
        {
            rx_clip_holder = (
                clipboard::get_rx_cliprdr_server(id),
                crate::SimpleCallOnReturn {
                    b: true,
                    f: Box::new(move || {
                        clipboard::remove_channel_by_conn_id(id);
                    }),
                },
            );
            rx_clip = rx_clip_holder.0.lock().await;
        }
        #[cfg(not(feature = "unix-file-copy-paste"))]
        {
            (_tx_clip, rx_clip) = mpsc::unbounded_channel::<i32>();
        }

        loop {
            tokio::select! {
                // biased; // video has higher priority // causing test_delay_timer failed while transferring big file

                Some(data) = rx_from_cm.recv() => {
                    match data {
                        ipc::Data::Authorize => {
                            conn.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::Click);
                            conn.require_2fa.take();
                            if !conn.send_logon_response_and_keep_alive().await {
                                break;
                            }
                            if conn.port_forward_socket.is_some() {
                                break;
                            }
                        }
                        ipc::Data::Close => {
                            conn.chat_unanswered = false; // seen
                            conn.file_transferred = false; //seen
                            conn.send_close_reason_no_retry("").await;
                            conn.on_close("connection manager", true).await;
                            break;
                        }
                        // The connection manager's window went away rather than a person
                        // disconnecting this peer. End the session exactly as above, but do not
                        // send the manual close reason: it is the one thing that stops the peer
                        // from retrying, and on a logout the retry is the whole point - it is
                        // what puts the peer back on the login screen a moment later.
                        #[cfg(target_os = "linux")]
                        ipc::Data::CmWindowClosed => {
                            conn.chat_unanswered = false; // seen
                            conn.file_transferred = false; //seen
                            conn.on_close("connection manager window closed", true).await;
                            break;
                        }
                        ipc::Data::CmErr(e) => {
                            if e != "expected" {
                                // cm closed before connection
                                conn.on_close(&format!("connection manager error: {}", e), false).await;
                                break;
                            }
                        }
                        ipc::Data::ChatMessage{text} => {
                            let mut misc = Misc::new();
                            misc.set_chat_message(ChatMessage {
                                text,
                                ..Default::default()
                            });
                            let mut msg_out = Message::new();
                            msg_out.set_misc(misc);
                            conn.send(msg_out).await;
                            conn.chat_unanswered = false;
                        }
                        ipc::Data::SwitchPermission{name, enabled} => {
                            log::info!("Change permission {} -> {}", name, enabled);
                            if &name == "keyboard" {
                                conn.keyboard = enabled;
                                conn.send_permission(Permission::Keyboard, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::super::clipboard_service::NAME,
                                        conn.inner.clone(), conn.can_sub_clipboard_service());
                                    #[cfg(feature = "unix-file-copy-paste")]
                                    s.write().unwrap().subscribe(
                                        super::super::clipboard_service::FILE_NAME,
                                        conn.inner.clone(),
                                        conn.can_sub_file_clipboard_service(),
                                    );
                                    s.write().unwrap().subscribe(
                                        NAME_CURSOR,
                                        conn.inner.clone(), enabled || conn.show_remote_cursor);
                                }
                            } else if &name == "clipboard" {
                                conn.clipboard = enabled;
                                conn.send_permission(Permission::Clipboard, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::super::clipboard_service::NAME,
                                        conn.inner.clone(), conn.can_sub_clipboard_service());
                                }
                            } else if &name == "audio" {
                                conn.audio = enabled;
                                conn.send_permission(Permission::Audio, enabled).await;
                                if conn.authorized {
                                    if let Some(s) = conn.server.upgrade() {
                                        if conn.is_authed_view_camera_conn() {
                                            if conn.voice_calling || !conn.audio_enabled() {
                                                s.write().unwrap().subscribe(
                                                    super::super::audio_service::NAME,
                                                    conn.inner.clone(), conn.audio_enabled());
                                            }
                                        } else {
                                            s.write().unwrap().subscribe(
                                                super::super::audio_service::NAME,
                                                conn.inner.clone(), conn.audio_enabled());
                                        }
                                    }
                                }
                            } else if &name == "file" {
                                conn.file = enabled;
                                conn.send_permission(Permission::File, enabled).await;
                                #[cfg(feature = "unix-file-copy-paste")]
                                if !enabled {
                                    conn.try_empty_file_clipboard();
                                }
                                #[cfg(feature = "unix-file-copy-paste")]
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::super::clipboard_service::FILE_NAME,
                                        conn.inner.clone(),
                                        conn.can_sub_file_clipboard_service(),
                                    );
                                }
                            } else if &name == "restart" {
                                conn.restart = enabled;
                                conn.send_permission(Permission::Restart, enabled).await;
                            } else if &name == "recording" {
                                conn.recording = enabled;
                                conn.send_permission(Permission::Recording, enabled).await;
                            } else if &name == "block_input" {
                                conn.block_input = enabled;
                                conn.send_permission(Permission::BlockInput, enabled).await;
                            } else if &name == "privacy_mode" {
                                // Keep permission state and runtime state consistent:
                                // when revoking the permission, try to leave privacy mode first.
                                // Otherwise we could end up in an inconsistent state where
                                // permission looks disabled while privacy mode is still active.
                                if !enabled && privacy_mode::is_in_privacy_mode() {
                                    if let Some(conn_id) = privacy_mode::get_privacy_mode_conn_id() {
                                        if conn_id == conn.inner.id() {
                                            let impl_key =
                                                privacy_mode::get_cur_impl_key().unwrap_or_default();
                                            let turn_off_res =
                                                privacy_mode::turn_off_privacy(conn_id, None);
                                            match turn_off_res {
                                                Some(Ok(_)) => {
                                                    let msg_out = crate::common::make_privacy_mode_msg(
                                                        back_notification::PrivacyModeState::PrvOffByPeer,
                                                        impl_key.clone(),
                                                    );
                                                    conn.send(msg_out).await;
                                                }
                                                _ => {
                                                    let msg_out = Self::turn_off_privacy_result_to_msg(
                                                        turn_off_res,
                                                        impl_key,
                                                    );
                                                    conn.send(msg_out).await;
                                                    // Turn-off failed, so revert CM's optimistic toggle
                                                    // and keep the previous permission value.
                                                    conn.send_to_cm(ipc::Data::SwitchPermission {
                                                        name: "privacy_mode".to_owned(),
                                                        enabled: conn.privacy_mode,
                                                    });
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                                conn.privacy_mode = enabled;
                                conn.send_permission(Permission::PrivacyMode, enabled).await;
                            }
                        }
                        ipc::Data::RawMessage(bytes) => {
                            allow_err!(conn.stream.send_raw(bytes).await);
                        }
                        #[cfg(target_os = "windows")]
                        ipc::Data::ClipboardFile(clip) => {
                            if !conn.is_remote() {
                                continue;
                            }
                            match clip {
                                clipboard::ClipboardFile::Files { files } => {
                                    let files = files.into_iter().map(|(f, s)| {
                                        (f, s as i64)
                                    }).collect::<Vec<_>>();
                                    conn.post_file_audit(
                                        FileAuditType::RemoteSend,
                                        "",
                                        files,
                                        json!({}),
                                    );
                                }
                                _ => {
                                    allow_err!(conn.stream.send(&clip_2_msg(clip)).await);
                                }
                            }
                        }
                        ipc::Data::PrivacyModeState((_, state, impl_key)) => {
                            let msg_out = match state {
                                privacy_mode::PrivacyModeState::OffSucceeded => {
                                    crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffSucceeded,
                                        impl_key,
                                    )
                                }
                                privacy_mode::PrivacyModeState::OffByPeer => {
                                    crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffByPeer,
                                        impl_key,
                                    )
                                }
                                privacy_mode::PrivacyModeState::OffUnknown => {
                                     crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffUnknown,
                                        impl_key,
                                    )
                                }
                            };
                            conn.send(msg_out).await;
                        }
                        #[cfg(windows)]
                        ipc::Data::DataPortableService(ipc::DataPortableService::RequestStart) => {
                            if let Err(e) = portable_client::start_portable_service(portable_client::StartPara::Direct) {
                                log::error!("Failed to start portable service from cm: {:?}", e);
                            }
                        }
                        #[cfg(feature = "flutter")]
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        ipc::Data::SwitchSidesBack => {
                            let mut misc = Misc::new();
                            misc.set_switch_back(SwitchBack::default());
                            let mut msg = Message::new();
                            msg.set_misc(misc);
                            conn.send(msg).await;
                        }
                        ipc::Data::VoiceCallResponse(accepted) => {
                            conn.handle_voice_call(accepted).await;
                        }
                        ipc::Data::CloseVoiceCall(_reason) => {
                            log::debug!("Close the voice call from the ipc.");
                            conn.close_voice_call().await;
                            // Notify the peer that we closed the voice call.
                            let msg = new_voice_call_request(false);
                            conn.send(msg).await;
                        }
                        ipc::Data::ReadJobInitResult { id, file_num, include_hidden, conn_id, result } => {
                            if conn_id == conn.inner.id() {
                                conn.handle_read_job_init_result(id, file_num, include_hidden, result).await;
                            }
                        }
                        ipc::Data::FileBlockFromCM { id, file_num, data, compressed, conn_id } => {
                            if conn_id == conn.inner.id() {
                                conn.handle_file_block_from_cm(id, file_num, data, compressed).await;
                            }
                        }
                        ipc::Data::FileReadDone { id, file_num, conn_id } => {
                            if conn_id == conn.inner.id() {
                                conn.handle_file_read_done(id, file_num).await;
                            }
                        }
                        ipc::Data::FileReadError { id, file_num, err, conn_id } => {
                            if conn_id == conn.inner.id() {
                                conn.handle_file_read_error(id, file_num, err).await;
                            }
                        }
                        ipc::Data::FileDigestFromCM { id, file_num, last_modified, file_size, is_resume, conn_id } => {
                            if conn_id == conn.inner.id() {
                                conn.handle_file_digest_from_cm(id, file_num, last_modified, file_size, is_resume).await;
                            }
                        }
                        ipc::Data::AllFilesResult { id, conn_id, path, result } => {
                            if conn_id == conn.inner.id() {
                                conn.handle_all_files_result(id, path, result).await;
                            }
                        }
                        _ => {}
                    }
                },
                res = conn.stream.next() => {
                    if let Some(res) = res {
                        match res {
                            Err(err) => {
                                conn.on_close(&err.to_string(), true).await;
                                break;
                            },
                            Ok(bytes) => {
                                last_recv_time = Instant::now();
                                conn.session_last_recv_time.as_mut().map(|t| *t.lock().unwrap() = Instant::now());
                                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                                    if !conn.on_message(msg_in).await {
                                        break;
                                    }
                                    if conn.port_forward_socket.is_some() && conn.authorized {
                                        log::info!("Port forward, last_test_delay is none: {}", conn.last_test_delay.is_none());
                                        // Avoid TestDelay reply injection into rdp data stream
                                        if conn.last_test_delay.is_none() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        conn.on_close("Reset by the peer", true).await;
                        break;
                    }
                },
                _ = conn.file_timer.tick() => {
                    if !conn.read_jobs.is_empty() {
                        conn.send_to_cm(ipc::Data::FileTransferLog(("transfer".to_string(), fs::serialize_transfer_jobs(&conn.read_jobs))));
                        match fs::handle_read_jobs(&mut conn.read_jobs, &mut conn.stream).await {
                            Ok(log) => {
                                if !log.is_empty() {
                                    conn.send_to_cm(ipc::Data::FileTransferLog(("transfer".to_string(), log)));
                                }
                            }
                            Err(err) =>  {
                                conn.on_close(&err.to_string(), false).await;
                                break;
                            }
                        }
                    } else {
                        conn.file_timer = crate::rustdesk_interval(time::interval_at(Instant::now() + SEC30, SEC30));
                    }
                }
                Ok(conns) = hbbs_rx.recv() => {
                    if conns.contains(&id) {
                        conn.send_close_reason_no_retry("Closed manually by web console").await;
                        conn.on_close("web console", true).await;
                        break;
                    }
                }
                Some((instant, value)) = rx_video.recv() => {
                    if !conn.video_ack_required {
                        if let Some(message::Union::VideoFrame(vf)) = &value.union {
                            video_service::notify_video_frame_fetched(vf.display as usize, id, Some(instant.into()));
                        }
                    }
                    let send_begin = video_service::qos_diag_verbose().then(Instant::now);
                    if let Err(err) = conn.stream.send(&value as &Message).await {
                        conn.on_close(&err.to_string(), false).await;
                        break;
                    }
                    if let Some(begin) = send_begin {
                        let blocked = begin.elapsed().as_millis() as u32;
                        conn.video_send_max_ms = conn.video_send_max_ms.max(blocked);
                        conn.video_send_sum_ms = conn.video_send_sum_ms.saturating_add(blocked);
                        conn.video_send_count += 1;
                    }
                },
                Some((instant, value)) = rx.recv() => {
                    let latency = instant.elapsed().as_millis() as i64;
                    #[allow(unused_mut)]
                    let mut msg = value;

                    if latency > 1000 {
                        match &msg.union {
                            Some(message::Union::AudioFrame(_)) => {
                                // log::info!("audio frame latency {}", instant.elapsed().as_secs_f32());
                                continue;
                            }
                            _ => {}
                        }
                    }
                    match &msg.union {
                        Some(message::Union::Misc(m)) => {
                            match &m.union {
                                Some(misc::Union::StopService(_)) => {
                                    conn.send_close_reason_no_retry("").await;
                                    conn.on_close("stop service", false).await;
                                    break;
                                }
                                _ => {},
                            }
                        }
                        Some(message::Union::PeerInfo(_pi)) => {
                            conn.refresh_video_display(None);
                            #[cfg(target_os = "macos")]
                            conn.retina.set_displays(&_pi.displays);
                        }
                        Some(message::Union::CursorPosition(pos)) => {
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            {
                                if conn.follow_remote_cursor {
                                    conn.handle_cursor_switch_display(pos.clone()).await;
                                }
                            }
                            #[cfg(target_os = "macos")]
                            if let Some(new_msg) = conn.retina.on_cursor_pos(&pos, conn.display_idx) {
                                msg = Arc::new(new_msg);
                            }
                        }
                        Some(message::Union::MultiClipboards(_multi_clipboards)) => {
                            #[cfg(not(target_os = "ios"))]
                            if let Some(msg_out) = crate::clipboard::get_msg_if_not_support_multi_clip(&conn.lr.version, &conn.lr.my_platform, _multi_clipboards) {
                                if let Err(err) = conn.stream.send(&msg_out).await {
                                    conn.on_close(&err.to_string(), false).await;
                                    break;
                                }
                                continue;
                            }
                        }
                        _ => {}
                    }

                    let msg: &Message = &msg;
                    if let Err(err) = conn.stream.send(msg).await {
                        conn.on_close(&err.to_string(), false).await;
                        break;
                    }
                },
                Some(data) = rx_from_authed.recv() => {
                    match data {
                        _ => {}
                    }
                }
                _ = second_timer.tick() => {
                    #[cfg(windows)]
                    conn.portable_check();
                    raii::AuthedConnID::check_wake_lock_on_setting_changed();
                    if let Some((instant, minute)) = conn.auto_disconnect_timer.as_ref() {
                        if instant.elapsed().as_secs() > minute * 60 {
                            conn.send_close_reason_no_retry("Connection failed due to inactivity").await;
                            conn.on_close("auto disconnect", true).await;
                            break;
                        }
                    }
                    if video_service::qos_diag_verbose() && conn.video_send_count > 0 {
                        // Joined with `qos_trace` on `t`: a probe that waits behind a
                        // blocked write is not a slow network.
                        log::debug!(
                            "qos_send t={} id={id} frames={} send_max={} send_sum={} queued={}",
                            hbb_common::get_time(),
                            conn.video_send_count,
                            conn.video_send_max_ms,
                            conn.video_send_sum_ms,
                            rx_video.len()
                        );
                        conn.video_send_max_ms = 0;
                        conn.video_send_sum_ms = 0;
                        conn.video_send_count = 0;
                    }
                    conn.file_remove_log_control.on_timer().drain(..).map(|x| conn.send_to_cm(x)).count();
                    #[cfg(feature = "hwcodec")]
                    conn.update_supported_encoding();
                }
                _ = test_delay_timer.tick() => {
                    if last_recv_time.elapsed() >= SEC30 {
                        conn.on_close("Timeout", true).await;
                        break;
                    }
                    // The control end will jump out of the loop after receiving LoginResponse and will not reply to the TestDelay
                    if conn.last_test_delay.is_none() && !(conn.port_forward_socket.is_some() && conn.authorized) {
                        conn.last_test_delay = Some(Instant::now());
                        let mut msg_out = Message::new();
                        msg_out.set_test_delay(TestDelay{
                            last_delay: conn.network_delay,
                            target_bitrate: video_service::VIDEO_QOS.lock().unwrap().bitrate(),
                            ..Default::default()
                        });
                        conn.send(msg_out.into()).await;
                    }
                    if conn.is_authed_remote_conn() || conn.view_camera {
                        if let Some(last_test_delay) = conn.last_test_delay {
                            video_service::VIDEO_QOS.lock().unwrap().user_delay_response_elapsed(id, last_test_delay.elapsed().as_millis());
                        }
                    }
                }
                clip_file = rx_clip.recv() => match clip_file {
                    Some(_clip) => {
                        #[cfg(feature = "unix-file-copy-paste")]
                        if crate::is_support_file_copy_paste(&conn.lr.version)
                        {
                            conn.handle_file_clip(_clip).await;
                        }
                    }
                    None => {
                        //
                    }
                },
            }
        }

        #[cfg(feature = "unix-file-copy-paste")]
        {
            conn.try_empty_file_clipboard();
        }

        if let Some(video_privacy_conn_id) = privacy_mode::get_privacy_mode_conn_id() {
            if video_privacy_conn_id == id {
                let _ = Self::turn_off_privacy_to_msg(id, String::new());
            }
        }
        video_service::notify_video_frame_fetched_by_conn_id(id, None);
        if conn.authorized {
            password::update_temporary_password();
        }
        if let Err(err) = conn.try_port_forward_loop(&mut rx_from_cm).await {
            conn.on_close(&err.to_string(), false).await;
            raii::AuthedConnID::check_remove_session(conn.inner.id(), conn.session_key());
        }

        conn.post_conn_audit(json!({
            "action": "close",
        }));
        if let Some(s) = conn.server.upgrade() {
            let mut s = s.write().unwrap();
            s.remove_connection(&conn.inner);
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            try_stop_record_cursor_pos();
        }
        conn.on_close("End", true).await;
        log::info!("#{} connection loop exited", id);
    }
}
