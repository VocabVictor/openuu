use super::*;

impl Connection {
    pub async fn handle_voice_call(&mut self, accepted: bool) {
        if let Some(ts) = self.voice_call_request_timestamp.take() {
            let msg = new_voice_call_response(ts.get(), accepted);
            if accepted {
                crate::audio_service::set_voice_call_input_device(
                    crate::get_default_sound_input(),
                    false,
                );
                self.send_to_cm(Data::StartVoiceCall);
            } else {
                self.send_to_cm(Data::CloseVoiceCall("".to_owned()));
            }
            self.send(msg).await;
            self.voice_calling = accepted;
            if self.is_authed_view_camera_conn() {
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::super::audio_service::NAME,
                        self.inner.clone(),
                        self.audio_enabled() && accepted,
                    );
                }
            }
        } else {
            log::warn!("Possible a voice call attack.");
        }
    }

    pub async fn close_voice_call(&mut self) {
        crate::audio_service::set_voice_call_input_device(None, true);
        // Notify the connection manager that the voice call has been closed.
        self.send_to_cm(Data::CloseVoiceCall("".to_owned()));
        self.voice_calling = false;
        if self.is_authed_view_camera_conn() {
            if let Some(s) = self.server.upgrade() {
                s.write()
                    .unwrap()
                    .subscribe(super::super::audio_service::NAME, self.inner.clone(), false);
            }
        }
    }

    pub(super) async fn update_options(&mut self, o: &OptionMessage) {
        log::info!("Option update: {:?}", o);
        if let Ok(q) = o.image_quality.enum_value() {
            let image_quality;
            if let ImageQuality::NotSet = q {
                if o.custom_image_quality > 0 {
                    image_quality = o.custom_image_quality;
                } else {
                    image_quality = -1;
                }
            } else {
                image_quality = q.value();
            }
            if image_quality > 0 {
                video_service::VIDEO_QOS
                    .lock()
                    .unwrap()
                    .user_image_quality(self.inner.id(), image_quality);
            }
        }
        if o.custom_fps > 0 {
            video_service::VIDEO_QOS
                .lock()
                .unwrap()
                .user_custom_fps(self.inner.id(), o.custom_fps as _);
        }
        if let Some(q) = o.supported_decoding.clone().take() {
            scrap::codec::Encoder::update(scrap::codec::EncodingUpdate::Update(self.inner.id(), q));
        }
        if let Ok(q) = o.lock_after_session_end.enum_value() {
            if q != BoolOption::NotSet {
                self.lock_after_session_end = q == BoolOption::Yes;
            }
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Ok(q) = o.show_remote_cursor.enum_value() {
            if q != BoolOption::NotSet {
                self.show_remote_cursor = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        NAME_CURSOR,
                        self.inner.clone(),
                        self.peer_keyboard_enabled() || self.show_remote_cursor,
                    );
                    s.write().unwrap().subscribe(
                        NAME_POS,
                        self.inner.clone(),
                        self.show_remote_cursor,
                    );
                }
            }
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Ok(q) = o.follow_remote_cursor.enum_value() {
            if q != BoolOption::NotSet {
                self.follow_remote_cursor = q == BoolOption::Yes;
            }
        }
        if let Ok(q) = o.follow_remote_window.enum_value() {
            if q != BoolOption::NotSet {
                self.follow_remote_window = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        NAME_WINDOW_FOCUS,
                        self.inner.clone(),
                        self.follow_remote_window,
                    );
                }
            }
        }
        if let Ok(q) = o.disable_audio.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_audio = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    if self.is_authed_view_camera_conn() {
                        if self.voice_calling || !self.audio_enabled() {
                            s.write().unwrap().subscribe(
                                super::super::audio_service::NAME,
                                self.inner.clone(),
                                self.audio_enabled(),
                            );
                        }
                    } else {
                        s.write().unwrap().subscribe(
                            super::super::audio_service::NAME,
                            self.inner.clone(),
                            self.audio_enabled(),
                        );
                    }
                }
            }
        }
        #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
        if let Ok(q) = o.enable_file_transfer.enum_value() {
            if q != BoolOption::NotSet {
                self.enable_file_transfer = q == BoolOption::Yes;
                #[cfg(target_os = "windows")]
                self.send_to_cm(ipc::Data::ClipboardFileEnabled(
                    self.file_transfer_enabled(),
                ));
                #[cfg(feature = "unix-file-copy-paste")]
                if !self.enable_file_transfer {
                    self.try_empty_file_clipboard();
                }
                #[cfg(feature = "unix-file-copy-paste")]
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::super::clipboard_service::FILE_NAME,
                        self.inner.clone(),
                        self.can_sub_file_clipboard_service(),
                    );
                }
            }
        }
        if let Ok(q) = o.disable_clipboard.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_clipboard = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::super::clipboard_service::NAME,
                        self.inner.clone(),
                        self.can_sub_clipboard_service(),
                    );
                }
            }
        }
        if let Ok(q) = o.disable_keyboard.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_keyboard = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::super::clipboard_service::NAME,
                        self.inner.clone(),
                        self.can_sub_clipboard_service(),
                    );
                    #[cfg(feature = "unix-file-copy-paste")]
                    s.write().unwrap().subscribe(
                        super::super::clipboard_service::FILE_NAME,
                        self.inner.clone(),
                        self.can_sub_file_clipboard_service(),
                    );
                    s.write().unwrap().subscribe(
                        NAME_CURSOR,
                        self.inner.clone(),
                        self.peer_keyboard_enabled() || self.show_remote_cursor,
                    );
                }
            }
        }
        // For compatibility with old versions ( < 1.2.4 ).
        if hbb_common::get_version_number(&self.lr.version)
            < hbb_common::get_version_number("1.2.4")
        {
            if let Ok(q) = o.privacy_mode.enum_value() {
                if self.keyboard {
                    match q {
                        BoolOption::Yes => {
                            self.turn_on_privacy("".to_owned()).await;
                        }
                        BoolOption::No => {
                            self.turn_off_privacy("".to_owned()).await;
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Ok(q) = o.block_input.enum_value() {
            if self.keyboard && self.block_input {
                match q {
                    BoolOption::Yes => {
                        self.tx_input.send(MessageInput::BlockOn).ok();
                    }
                    BoolOption::No => {
                        self.tx_input.send(MessageInput::BlockOff).ok();
                    }
                    _ => {}
                }
            } else {
                if q != BoolOption::NotSet {
                    let state = if q == BoolOption::Yes {
                        back_notification::BlockInputState::BlkOnFailed
                    } else {
                        back_notification::BlockInputState::BlkOffFailed
                    };
                    if let Some(tx) = &self.inner.tx {
                        Self::send_block_input_error(tx, state, "No permission".to_string());
                    }
                }
            }
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Ok(q) = o.terminal_persistent.enum_value() {
            if q != BoolOption::NotSet {
                self.update_terminal_persistence(q == BoolOption::Yes).await;
            }
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Ok(q) = o.show_my_cursor.enum_value() {
            if q != BoolOption::NotSet {
                use crate::whiteboard;
                self.show_my_cursor = q == BoolOption::Yes;
                #[cfg(target_os = "windows")]
                let is_lower_win10 = !crate::platform::windows::is_win_10_or_greater();
                #[cfg(not(target_os = "windows"))]
                let is_lower_win10 = false;
                #[cfg(target_os = "linux")]
                let is_linux_supported = crate::whiteboard::is_supported();
                #[cfg(not(target_os = "linux"))]
                let is_linux_supported = false;
                let not_support_msg = if is_lower_win10 {
                    "Windows 10 or greater is required."
                } else if cfg!(target_os = "linux") && !is_linux_supported {
                    "This feature is not supported on native Wayland, please install XWayland or switch to X11."
                } else {
                    ""
                };
                if q == BoolOption::Yes {
                    if not_support_msg.is_empty() {
                        whiteboard::register_whiteboard(whiteboard::get_key_cursor(self.inner.id));
                    } else {
                        let mut msg_out = Message::new();
                        let res = MessageBox {
                            msgtype: "nook-nocancel-hasclose".to_owned(),
                            title: "Show my cursor".to_owned(),
                            text: not_support_msg.to_owned(),
                            link: "".to_owned(),
                            ..Default::default()
                        };
                        msg_out.set_message_box(res);
                        self.send(msg_out).await;
                    }
                } else {
                    if not_support_msg.is_empty() {
                        whiteboard::unregister_whiteboard(whiteboard::get_key_cursor(
                            self.inner.id,
                        ));
                    }
                }
            }
        }
    }
}
