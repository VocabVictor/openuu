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
                        // A key frame starts a new reference chain: what is
                        // queued is older than it and no longer decodable.
                        thread.video_queue.clear();
                        thread
                            .video_sender
                            .send(MediaData::VideoFrame(Box::new(vf)))
                            .ok();
                    } else {
                        // A dropped frame breaks the reference chain, so
                        // the peer has to send a key frame before the
                        // picture is whole again. The token goes out either
                        // way: one token per queued frame keeps the decode
                        // thread and the queue in step.
                        if thread.video_queue.push(vf).needs_key_frame() {
                            self.handler.refresh_video(display as _);
                        }
                        thread.video_sender.send(MediaData::VideoQueue).ok();
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
                    self.handle_clipboard(cb);
                }
                Some(message::Union::MultiClipboards(mcb)) => {
                    self.handle_multi_clipboards(mcb);
                }
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                Some(message::Union::Cliprdr(clip)) => {
                    self.handle_cliprdr_msg(clip, peer).await;
                }
                Some(message::Union::FileResponse(fr)) => {
                    self.handle_file_response(fr, peer).await;
                }
                Some(message::Union::Misc(misc)) => {
                    if !self.handle_misc(misc).await {
                        return false;
                    }
                }
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
                Some(message::Union::FileAction(action)) => {
                    self.handle_file_action(action).await;
                }
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
