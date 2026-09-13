use super::*;

impl<T: InvokeUiSession> Remote<T> {
    pub(super) async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::VideoFrame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                        self.handler.close_success();
                        self.handler.adapt_size();
                        self.send_toggle_virtual_display_msg(peer).await;
                        self.send_toggle_privacy_mode_msg(peer).await;
                    }
                    self.video_format = CodecFormat::from(&vf);

                    let display = vf.display as usize;
                    if !self.video_threads.contains_key(&display) {
                        self.new_video_thread(display);
                    }
                    let Some(thread) = self.video_threads.get_mut(&display) else {
                        return true;
                    };
                    if Self::contains_key_frame(&vf) {
                        thread
                            .video_sender
                            .send(MediaData::VideoFrame(Box::new(vf)))
                            .ok();
                    } else {
                        let video_queue = thread.video_queue.read().unwrap();
                        if video_queue.force_push(vf).is_some() {
                            drop(video_queue);
                            self.handler.refresh_video(display as _);
                        } else {
                            thread.video_sender.send(MediaData::VideoQueue).ok();
                        }
                    }
                }
                Some(message::Union::Hash(hash)) => {
                    if !self
                        .handler
                        .handle_hash(&self.handler.password.clone(), hash, peer)
                        .await
                    {
                        return false;
                    }
                }
                Some(message::Union::LoginResponse(lr)) => match lr.union {
                    Some(login_response::Union::Error(err)) => {
                        if err == client::REQUIRE_2FA {
                            self.handler.lc.write().unwrap().enable_trusted_devices =
                                lr.enable_trusted_devices;
                        }
                        if !self.handler.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::PeerInfo(pi)) => {
                        let peer_version = pi.version.clone();
                        let peer_platform = pi.platform.clone();
                        self.set_peer_info(&pi);
                        if self.handler.is_view_camera() {
                            if !self.check_view_camera_support(&peer_version, &peer_platform) {
                                self.handler.lc.write().unwrap().handle_peer_info(&pi);
                                return false;
                            }
                        }
                        if self.handler.is_terminal() {
                            if !self.check_terminal_support(&peer_version) {
                                self.handler.lc.write().unwrap().handle_peer_info(&pi);
                                return false;
                            }
                        }
                        self.handler.handle_peer_info(pi);
                        if self.handler.is_default() {
                            #[cfg(feature = "flutter")]
                            #[cfg(not(target_os = "ios"))]
                            let rx = Client::try_start_clipboard(None);
                            // To make sure current text clipboard data is updated.
                            #[cfg(not(target_os = "ios"))]
                            if let Some(mut rx) = rx {
                                timeout(CLIPBOARD_INTERVAL, rx.recv()).await.ok();
                            }

                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            if self.handler.lc.read().unwrap().sync_init_clipboard.v {
                                if let Some(msg_out) = crate::clipboard::get_current_clipboard_msg(
                                    &peer_version,
                                    &peer_platform,
                                    crate::clipboard::ClipboardSide::Client,
                                ) {
                                    let sender = self.sender.clone();
                                    let permission_config = self.handler.get_permission_config();
                                    tokio::spawn(async move {
                                        if permission_config.is_text_clipboard_required() {
                                            sender.send(Data::Message(msg_out)).ok();
                                        }
                                    });
                                }
                            }
                            // to-do: Android, is `sync_init_clipboard` really needed?
                            // https://github.com/rustdesk/rustdesk/discussions/9010

                            #[cfg(feature = "flutter")]
                            #[cfg(not(target_os = "ios"))]
                            crate::flutter::update_text_clipboard_required();

                            #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
                            crate::flutter::update_file_clipboard_required();
                        }

                        if self.handler.is_file_transfer() {
                            self.handler.load_last_jobs();
                        }

                        self.is_connected = true;
                    }
                    _ => {}
                },
                Some(message::Union::CursorData(cd)) => {
                    self.handler.set_cursor_data(cd);
                }
                Some(message::Union::CursorId(id)) => {
                    self.handler.set_cursor_id(id.to_string());
                }
                Some(message::Union::CursorPosition(cp)) => {
                    self.handler.set_cursor_position(cp);
                }
                Some(message::Union::Clipboard(cb)) => {
                    let clipboard_allowed = {
                        let lc = self.handler.lc.read().unwrap();
                        !lc.disable_clipboard.v && !lc.get_toggle_option("view-only")
                    };
                    if clipboard_allowed {
                        #[cfg(all(
                            feature = "flutter",
                            not(any(target_os = "android", target_os = "ios"))
                        ))]
                        if self.handler.is_text_clipboard_required()
                            && crate::clipboard::is_sync_clipboard_between_sessions_enabled()
                        {
                            let mut msg = Message::new();
                            msg.set_clipboard(cb.clone());
                            let session_id = self.handler.lc.read().unwrap().session_id;
                            crate::flutter::send_clipboard_msg_to_other_sessions(msg, session_id);
                        }
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(vec![cb], ClipboardSide::Client);
                        #[cfg(target_os = "ios")]
                        {
                            let content = if cb.compress {
                                hbb_common::compress::decompress(&cb.content)
                            } else {
                                cb.content.into()
                            };
                            if let Ok(content) = String::from_utf8(content) {
                                self.handler.clipboard(content);
                            }
                        }
                        #[cfg(target_os = "android")]
                        crate::clipboard::handle_msg_clipboard(cb);
                    }
                }
                Some(message::Union::MultiClipboards(_mcb)) => {
                    let clipboard_allowed = {
                        let lc = self.handler.lc.read().unwrap();
                        !lc.disable_clipboard.v && !lc.get_toggle_option("view-only")
                    };
                    if clipboard_allowed {
                        #[cfg(all(
                            feature = "flutter",
                            not(any(target_os = "android", target_os = "ios"))
                        ))]
                        if self.handler.is_text_clipboard_required()
                            && crate::clipboard::is_sync_clipboard_between_sessions_enabled()
                        {
                            let mut msg = Message::new();
                            msg.set_multi_clipboards(_mcb.clone());
                            let session_id = self.handler.lc.read().unwrap().session_id;
                            crate::flutter::send_clipboard_msg_to_other_sessions(msg, session_id);
                        }
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(_mcb.clipboards, ClipboardSide::Client);
                        #[cfg(target_os = "ios")]
                        {
                            if let Some(cb) = _mcb
                                .clipboards
                                .iter()
                                .find(|c| c.format.enum_value() == Ok(ClipboardFormat::Text))
                            {
                                let content = if cb.compress {
                                    hbb_common::compress::decompress(&cb.content)
                                } else {
                                    cb.content.to_vec()
                                };
                                if let Ok(content) = String::from_utf8(content) {
                                    self.handler.clipboard(content);
                                }
                            }
                        }
                        #[cfg(target_os = "android")]
                        crate::clipboard::handle_msg_multi_clipboards(_mcb);
                    }
                }
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                Some(message::Union::Cliprdr(clip)) => {
                    self.handle_cliprdr_msg(clip, peer).await;
                }
                Some(message::Union::FileResponse(fr)) => {
                    match fr.union {
                        Some(file_response::Union::EmptyDirs(res)) => {
                            self.handler.update_empty_dirs(res);
                        }
                        Some(file_response::Union::Dir(fd)) => {
                            #[cfg(windows)]
                            let entries = fd.entries.to_vec();
                            #[cfg(not(windows))]
                            let mut entries = fd.entries.to_vec();
                            #[cfg(not(windows))]
                            {
                                if self.handler.peer_platform() == "Windows" {
                                    fs::transform_windows_path(&mut entries);
                                }
                            }
                            // We cannot call cancel_transfer_job/handle_job_status while holding
                            // a mutable borrow from fs::get_job(&mut self.write_jobs), so defer
                            // the error handling until after the borrow scope ends.
                            let mut set_files_err = None;
                            if let Some(job) = fs::get_job(fd.id, &mut self.write_jobs) {
                                log::info!("job set_files: {:?}", entries);
                                if let Err(err) = job.set_files(entries) {
                                    set_files_err = Some(err.to_string());
                                } else {
                                    job.set_finished_size_on_resume();
                                    self.handler.update_folder_files(
                                        fd.id,
                                        job.files(),
                                        fd.path,
                                        false,
                                        false,
                                    );
                                }
                            } else if let Some(job) = self.remove_jobs.get_mut(&fd.id) {
                                // Intentionally keep raw entries here:
                                // - remote remove flow executes deletions on peer side;
                                // - local remove flow is populated from local get_recursive_files().
                                job.files = entries;
                                self.handler
                                    .update_folder_files(fd.id, &job.files, fd.path, false, false);
                            } else {
                                self.handler
                                    .update_folder_files(fd.id, &entries, fd.path, false, false);
                            }
                            if let Some(err) = set_files_err {
                                log::warn!(
                                    "Rejected unsafe file list from remote peer for job {}: {}",
                                    fd.id,
                                    err
                                );
                                self.cancel_transfer_job(fd.id, peer).await;
                                self.handle_job_status(fd.id, -1, Some(err));
                            }
                        }
                        Some(file_response::Union::Digest(digest)) => {
                            if digest.is_upload {
                                if let Some(job) = fs::get_job(digest.id, &mut self.read_jobs) {
                                    if let Some(file) = job.files().get(digest.file_num as usize) {
                                        if let fs::DataSource::FilePath(p) = &job.data_source {
                                            let read_path =
                                                get_string(&fs::TransferJob::join(p, &file.name));
                                            let mut overwrite_strategy =
                                                job.default_overwrite_strategy();
                                            let mut offset = 0;
                                            if digest.is_identical && job.is_resume {
                                                if digest.transferred_size > 0 {
                                                    overwrite_strategy = Some(true);
                                                    offset = digest.transferred_size as _;
                                                }
                                            }
                                            if let Some(overwrite) = overwrite_strategy {
                                                let req = FileTransferSendConfirmRequest {
                                                    id: digest.id,
                                                    file_num: digest.file_num,
                                                    union: Some(if overwrite {
                                                        file_transfer_send_confirm_request::Union::OffsetBlk(offset)
                                                    } else {
                                                        file_transfer_send_confirm_request::Union::Skip(
                                                            true,
                                                        )
                                                    }),
                                                    ..Default::default()
                                                };
                                                job.confirm(&req).await;
                                                let msg = new_send_confirm(req);
                                                allow_err!(peer.send(&msg).await);
                                            } else {
                                                self.handler.override_file_confirm(
                                                    digest.id,
                                                    digest.file_num,
                                                    read_path,
                                                    true,
                                                    digest.is_identical,
                                                );
                                            }
                                        }
                                    }
                                }
                            } else {
                                if let Some(job) = fs::get_job(digest.id, &mut self.write_jobs) {
                                    if let Some(file) = job.files().get(digest.file_num as usize) {
                                        if let fs::DataSource::FilePath(p) = &job.data_source {
                                            let write_path =
                                                get_string(&fs::TransferJob::join(p, &file.name));
                                            job.set_file_digest(digest.file_num, digest.file_size, digest.last_modified);
                                            let peer_ver = self.handler.lc.read().unwrap().version;
                                            let is_support_resume =
                                                crate::is_support_file_transfer_resume_num(
                                                    peer_ver,
                                                );
                                            match fs::is_write_need_confirmation(
                                                is_support_resume && job.is_resume,
                                                &write_path,
                                                &digest,
                                            ) {
                                                Ok(res) => match res {
                                                    DigestCheckResult::IsSame => {
                                                        let req = FileTransferSendConfirmRequest {
                                                            id: digest.id,
                                                            file_num: digest.file_num,
                                                            union: Some(file_transfer_send_confirm_request::Union::Skip(true)),
                                                            ..Default::default()
                                                        };
                                                        job.confirm(&req).await;
                                                        let msg = new_send_confirm(req);
                                                        allow_err!(peer.send(&msg).await);
                                                    }
                                                    DigestCheckResult::NeedConfirm(digest) => {
                                                        let mut overwrite_strategy =
                                                            job.default_overwrite_strategy();
                                                        let mut offset = 0;
                                                        if digest.is_identical
                                                            && job.is_resume
                                                            && digest.transferred_size > 0
                                                        {
                                                            overwrite_strategy = Some(true);
                                                            offset = digest.transferred_size as _;
                                                        }
                                                        if let Some(overwrite) = overwrite_strategy
                                                        {
                                                            let req =
                                                                FileTransferSendConfirmRequest {
                                                                    id: digest.id,
                                                                    file_num: digest.file_num,
                                                                    union: Some(if overwrite {
                                                                        file_transfer_send_confirm_request::Union::OffsetBlk(offset)
                                                                    } else {
                                                                        file_transfer_send_confirm_request::Union::Skip(true)
                                                                    }),
                                                                    ..Default::default()
                                                                };
                                                            job.confirm(&req).await;
                                                            let msg = new_send_confirm(req);
                                                            allow_err!(peer.send(&msg).await);
                                                        } else {
                                                            self.handler.override_file_confirm(
                                                                digest.id,
                                                                digest.file_num,
                                                                write_path,
                                                                false,
                                                                digest.is_identical,
                                                            );
                                                        }
                                                    }
                                                    DigestCheckResult::NoSuchFile => {
                                                        let req = FileTransferSendConfirmRequest {
                                                        id: digest.id,
                                                        file_num: digest.file_num,
                                                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                                                        ..Default::default()
                                                    };
                                                        job.confirm(&req).await;
                                                        let msg = new_send_confirm(req);
                                                        allow_err!(peer.send(&msg).await);
                                                    }
                                                },
                                                Err(err) => {
                                                    println!("error receiving digest: {}", err);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(file_response::Union::Block(block)) => {
                            if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                                if let Err(_err) = job.write(block).await {
                                    // to-do: add "skip" for writing job
                                }
                                if job.r#type == fs::JobType::Generic {
                                    self.update_jobs_status();
                                }
                            }
                        }
                        Some(file_response::Union::Done(d)) => {
                            let mut err: Option<String> = None;
                            let mut job_type = fs::JobType::Generic;
                            if let Some(job) = fs::remove_job(d.id, &mut self.write_jobs) {
                                job.modify_time();
                                err = job.job_error();
                                job_type = job.r#type;
                            }
                            match job_type {
                                fs::JobType::Generic => {
                                    self.handle_job_status(d.id, d.file_num, err);
                                }
                                fs::JobType::Printer => {}
                            }
                        }
                        Some(file_response::Union::Error(e)) => {
                            let job_type = fs::remove_job(e.id, &mut self.write_jobs)
                                .or_else(|| fs::remove_job(e.id, &mut self.read_jobs))
                                .map(|j| j.r#type)
                                .unwrap_or(fs::JobType::Generic);
                            match job_type {
                                fs::JobType::Generic => {
                                    self.handle_job_status(e.id, e.file_num, Some(e.error));
                                }
                                fs::JobType::Printer => {
                                    log::debug!("Discarded obsolete transfer job {}", e.id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Some(message::Union::Misc(misc)) => match misc.union {
                    Some(misc::Union::AudioFormat(f)) => {
                        self.audio_sender.send(MediaData::AudioFormat(f)).ok();
                    }
                    Some(misc::Union::ChatMessage(c)) => {
                        self.handler.new_message(c.text);
                    }
                    Some(misc::Union::PermissionInfo(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
                        // https://github.com/rustdesk/rustdesk/issues/3703#issuecomment-1474734754
                        match p.permission.enum_value() {
                            Ok(Permission::Keyboard) => {
                                *self.handler.server_keyboard_enabled.write().unwrap() = p.enabled;
                                #[cfg(feature = "flutter")]
                                #[cfg(not(target_os = "ios"))]
                                crate::flutter::update_text_clipboard_required();
                                #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
                                crate::flutter::update_file_clipboard_required();
                                self.handler.set_permission("keyboard", p.enabled);
                            }
                            Ok(Permission::Clipboard) => {
                                *self.handler.server_clipboard_enabled.write().unwrap() = p.enabled;
                                #[cfg(feature = "flutter")]
                                #[cfg(not(target_os = "ios"))]
                                crate::flutter::update_text_clipboard_required();
                                self.handler.set_permission("clipboard", p.enabled);
                            }
                            Ok(Permission::Audio) => {
                                self.handler.set_permission("audio", p.enabled);
                            }
                            Ok(Permission::File) => {
                                *self.handler.server_file_transfer_enabled.write().unwrap() =
                                    p.enabled;
                                if !p.enabled && self.handler.is_file_transfer() {
                                    return true;
                                }
                                #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
                                crate::flutter::update_file_clipboard_required();
                                self.handler.set_permission("file", p.enabled);
                                #[cfg(feature = "unix-file-copy-paste")]
                                if !p.enabled {
                                    try_empty_clipboard_files(
                                        ClipboardSide::Client,
                                        self.client_conn_id,
                                    );
                                }
                            }
                            Ok(Permission::Restart) => {
                                self.handler.set_permission("restart", p.enabled);
                            }
                            Ok(Permission::Recording) => {
                                self.handler.lc.write().unwrap().record_permission = p.enabled;
                                self.update_record_state();
                                self.handler.set_permission("recording", p.enabled);
                            }
                            Ok(Permission::BlockInput) => {
                                self.handler.set_permission("block_input", p.enabled);
                            }
                            Ok(Permission::PrivacyMode) => {
                                self.handler.set_permission("privacy_mode", p.enabled);
                            }
                            _ => {}
                        }
                    }
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.handler.handle_peer_switch_display(&s);
                        if let Some(thread) = self.video_threads.get_mut(&(s.display as usize)) {
                            thread.video_sender.send(MediaData::Reset).ok();
                        }

                        let mut scale = 1.0;
                        if let Some(pi) = &self.handler.lc.read().unwrap().peer_info {
                            if let Some(d) = pi.displays.get(s.display as usize) {
                                scale = d.scale;
                            }
                        }

                        if s.width > 0 && s.height > 0 {
                            self.handler.set_display(
                                s.x,
                                s.y,
                                s.width,
                                s.height,
                                s.cursor_embedded,
                                scale,
                            );
                        }
                    }
                    Some(misc::Union::CloseReason(c)) => {
                        self.sent_close_reason = true; // The controlled end will close, no need to send close reason
                        self.handler.msgbox("error", "Connection Error", &c, "");
                        return false;
                    }
                    Some(misc::Union::BackNotification(notification)) => {
                        if !self.handle_back_notification(notification).await {
                            return false;
                        }
                    }
                    Some(misc::Union::Uac(uac)) => {
                        let keyboard = self.handler.server_keyboard_enabled.read().unwrap().clone();
                        #[cfg(feature = "flutter")]
                        {
                            if uac && keyboard {
                                self.handler.msgbox(
                                    "on-uac",
                                    "Prompt",
                                    "Please wait for confirmation of UAC...",
                                    "",
                                );
                            } else {
                                self.handler.cancel_msgbox("on-uac");
                                self.handler.cancel_msgbox("wait-uac");
                                self.handler.cancel_msgbox("elevation-error");
                            }
                        }
                    }
                    Some(misc::Union::ForegroundWindowElevated(elevated)) => {
                        let keyboard = self.handler.server_keyboard_enabled.read().unwrap().clone();
                        #[cfg(feature = "flutter")]
                        {
                            if elevated && keyboard {
                                self.handler.msgbox(
                                    "on-foreground-elevated",
                                    "Prompt",
                                    "elevated_foreground_window_tip",
                                    "",
                                );
                            } else {
                                self.handler.cancel_msgbox("on-foreground-elevated");
                                self.handler.cancel_msgbox("wait-uac");
                                self.handler.cancel_msgbox("elevation-error");
                            }
                        }
                    }
                    Some(misc::Union::ElevationResponse(err)) => {
                        if err.is_empty() {
                            self.handler.msgbox("wait-uac", "", "", "");
                        } else {
                            self.handler.cancel_msgbox("wait-uac");
                            self.handler
                                .msgbox("elevation-error", "Elevation Error", &err, "");
                        }
                    }
                    Some(misc::Union::PortableServiceRunning(b)) => {
                        self.handler.portable_service_running(b);
                        if self.elevation_requested && b {
                            self.handler.msgbox(
                                "custom-nocancel-success",
                                "Successful",
                                "Elevate successfully",
                                "",
                            );
                        }
                    }
                    #[cfg(feature = "flutter")]
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::SwitchBack(_)) => {
                        let allow_switch_back = self
                            .handler
                            .lc
                            .write()
                            .unwrap()
                            .consume_switch_back_permission();
                        if allow_switch_back {
                            self.handler.switch_back(&self.handler.get_id());
                        } else {
                            log::warn!(
                                "Ignored unsolicited SwitchBack from {}",
                                self.handler.get_id()
                            );
                        }
                    }
                    Some(misc::Union::SupportedEncoding(e)) => {
                        log::info!("update supported encoding:{:?}", e);
                        self.handler.lc.write().unwrap().supported_encoding = e;
                    }
                    Some(misc::Union::QuickLaunchResponse(response)) => {
                        if response.len() <= 2 * 1024 * 1024 {
                            self.handler.ui_handler.quick_launch_response(response);
                        }
                    }
                    Some(misc::Union::FollowCurrentDisplay(d_idx)) => {
                        self.handler.set_current_display(d_idx);
                    }
                    _ => {}
                },
                Some(message::Union::TestDelay(t)) => {
                    self.handler.handle_test_delay(t, peer).await;
                }
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.handler.lc.read().unwrap().disable_audio.v {
                        self.audio_sender
                            .send(MediaData::AudioFrame(Box::new(frame)))
                            .ok();
                    }
                }
                Some(message::Union::FileAction(action)) => match action.union {
                    Some(file_action::Union::Send(_s)) => match _s.file_type.enum_value() {
                        #[cfg(target_os = "windows")]
                        Ok(file_transfer_send_request::FileType::Printer) => {}
                        _ => {}
                    },
                    Some(file_action::Union::SendConfirm(c)) => {
                        if let Some(job) = fs::get_job(c.id, &mut self.read_jobs) {
                            job.confirm(&c).await;
                        }
                    }
                    _ => {}
                },
                Some(message::Union::MessageBox(msgbox)) => self.handle_message_box(msgbox),
                Some(message::Union::VoiceCallRequest(request)) => self.handle_voice_call_request(request),
                Some(message::Union::VoiceCallResponse(response)) => self.handle_voice_call_response(response),
                Some(message::Union::PeerInfo(pi)) => self.handle_bare_peer_info(pi),
                Some(message::Union::ScreenshotResponse(response)) => self.handle_screenshot_response(response),
                Some(message::Union::TerminalResponse(response)) => self.handle_terminal_response(response),
                _ => {}
            }
        }
        true
    }
}
