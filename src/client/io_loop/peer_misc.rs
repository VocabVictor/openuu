use super::*;

impl<T: InvokeUiSession> Remote<T> {
    /// Handle one `Misc` message; `false` ends the io loop.
    pub(super) async fn handle_misc(&mut self, misc: Misc) -> bool {
        match misc.union {
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
        }
        true
    }
}
