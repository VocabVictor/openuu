use super::*;

impl Connection {
    /// Apply a permission toggle from the connection manager.
    pub(super) async fn handle_switch_permission(&mut self, name: String, enabled: bool) {
        log::info!("Change permission {} -> {}", name, enabled);
        if &name == "keyboard" {
            self.keyboard = enabled;
            self.send_permission(Permission::Keyboard, enabled).await;
            if let Some(s) = self.server.upgrade() {
                s.write().unwrap().subscribe(
                    super::super::clipboard_service::NAME,
                    self.inner.clone(), self.can_sub_clipboard_service());
                #[cfg(feature = "unix-file-copy-paste")]
                s.write().unwrap().subscribe(
                    super::super::clipboard_service::FILE_NAME,
                    self.inner.clone(),
                    self.can_sub_file_clipboard_service(),
                );
                s.write().unwrap().subscribe(
                    NAME_CURSOR,
                    self.inner.clone(), enabled || self.show_remote_cursor);
            }
        } else if &name == "clipboard" {
            self.clipboard = enabled;
            self.send_permission(Permission::Clipboard, enabled).await;
            if let Some(s) = self.server.upgrade() {
                s.write().unwrap().subscribe(
                    super::super::clipboard_service::NAME,
                    self.inner.clone(), self.can_sub_clipboard_service());
            }
        } else if &name == "audio" {
            self.audio = enabled;
            self.send_permission(Permission::Audio, enabled).await;
            if self.authorized {
                if let Some(s) = self.server.upgrade() {
                    if self.is_authed_view_camera_conn() {
                        if self.voice_calling || !self.audio_enabled() {
                            s.write().unwrap().subscribe(
                                super::super::audio_service::NAME,
                                self.inner.clone(), self.audio_enabled());
                        }
                    } else {
                        s.write().unwrap().subscribe(
                            super::super::audio_service::NAME,
                            self.inner.clone(), self.audio_enabled());
                    }
                }
            }
        } else if &name == "file" {
            self.file = enabled;
            self.send_permission(Permission::File, enabled).await;
            #[cfg(feature = "unix-file-copy-paste")]
            if !enabled {
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
        } else if &name == "restart" {
            self.restart = enabled;
            self.send_permission(Permission::Restart, enabled).await;
        } else if &name == "recording" {
            self.recording = enabled;
            self.send_permission(Permission::Recording, enabled).await;
        } else if &name == "block_input" {
            self.block_input = enabled;
            self.send_permission(Permission::BlockInput, enabled).await;
        } else if &name == "privacy_mode" {
            // Keep permission state and runtime state consistent:
            // when revoking the permission, try to leave privacy mode first.
            // Otherwise we could end up in an inconsistent state where
            // permission looks disabled while privacy mode is still active.
            if !enabled && privacy_mode::is_in_privacy_mode() {
                if let Some(conn_id) = privacy_mode::get_privacy_mode_conn_id() {
                    if conn_id == self.inner.id() {
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
                                self.send(msg_out).await;
                            }
                            _ => {
                                let msg_out = Self::turn_off_privacy_result_to_msg(
                                    turn_off_res,
                                    impl_key,
                                );
                                self.send(msg_out).await;
                                // Turn-off failed, so revert CM's optimistic toggle
                                // and keep the previous permission value.
                                self.send_to_cm(ipc::Data::SwitchPermission {
                                    name: "privacy_mode".to_owned(),
                                    enabled: self.privacy_mode,
                                });
                                return;
                            }
                        }
                    }
                }
            }
            self.privacy_mode = enabled;
            self.send_permission(Permission::PrivacyMode, enabled).await;
        }
    }
}
