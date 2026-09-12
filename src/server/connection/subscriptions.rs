use super::*;

impl Connection {
    pub(super) fn try_sub_camera_displays(&mut self) {
        if let Some(s) = self.server.upgrade() {
            let mut s = s.write().unwrap();

            s.try_add_primary_camera_service();
            s.add_camera_connection(self.inner.clone());
        }
    }

    #[inline]
    pub(super) fn is_remote(&self) -> bool {
        self.file_transfer.is_none()
            && !self.is_port_forward()
            && !self.view_camera
            && !self.terminal
    }

    #[inline]
    pub(super) fn is_port_forward(&self) -> bool {
        self.port_forward_socket.is_some() || self.port_forward_mux.is_some()
    }

    pub(super) fn try_sub_monitor_services(&mut self) {
        let is_remote = self.is_remote();
        if is_remote && !self.services_subed {
            self.services_subed = true;
            if let Some(s) = self.server.upgrade() {
                let mut noperms = Vec::new();
                if !self.peer_keyboard_enabled() && !self.show_remote_cursor {
                    noperms.push(NAME_CURSOR);
                }
                if !self.show_remote_cursor {
                    noperms.push(NAME_POS);
                }
                if !self.follow_remote_window {
                    noperms.push(NAME_WINDOW_FOCUS);
                }
                if !self.can_sub_clipboard_service() {
                    noperms.push(super::super::clipboard_service::NAME);
                }
                #[cfg(feature = "unix-file-copy-paste")]
                if !self.can_sub_file_clipboard_service() {
                    noperms.push(super::super::clipboard_service::FILE_NAME);
                }
                if !self.audio_enabled() {
                    noperms.push(super::super::audio_service::NAME);
                }
                let mut s = s.write().unwrap();
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                let _h = try_start_record_cursor_pos();
                self.auto_disconnect_timer = Self::get_auto_disconenct_timer();
                s.try_add_monitor_service(self.display_idx);
                s.add_monitor_connection(self.inner.clone(), &noperms, self.display_idx);
            }
        }
    }

    #[cfg(windows)]
    pub(super) fn handle_windows_specific_session(
        &mut self,
        pi: &mut PeerInfo,
        wait_session_id_confirm: &mut bool,
    ) {
        let sessions = crate::platform::get_available_sessions(true);
        if let Some(current_sid) = crate::platform::get_current_process_session_id() {
            if crate::platform::is_installed()
                && crate::platform::is_share_rdp()
                && raii::AuthedConnID::non_port_forward_conn_count() == 1
                && sessions.len() > 1
                && sessions.iter().any(|e| e.sid == current_sid)
                && get_version_number(&self.lr.version) >= get_version_number("1.2.4")
            {
                pi.windows_sessions = Some(WindowsSessions {
                    sessions,
                    current_sid,
                    ..Default::default()
                })
                .into();
                *wait_session_id_confirm = true;
            }
        }
    }

    pub(super) fn on_remote_authorized(&self) {
        self.update_codec_on_login();
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        if config::option2bool(
            "allow-remove-wallpaper",
            &Config::get_option("allow-remove-wallpaper"),
        ) {
            // multi connections set once
            let mut wallpaper = WALLPAPER_REMOVER.lock().unwrap();
            if wallpaper.is_none() {
                match crate::platform::WallPaperRemover::new() {
                    Ok(remover) => {
                        *wallpaper = Some(remover);
                    }
                    Err(e) => {
                        log::info!("create wallpaper remover failed: {:?}", e);
                    }
                }
            }
        }
    }

    pub(super) fn peer_keyboard_enabled(&self) -> bool {
        self.keyboard && !self.disable_keyboard
    }

    pub(super) fn clipboard_enabled(&self) -> bool {
        self.clipboard && !self.disable_clipboard
    }

    #[inline]
    pub(super) fn can_sub_clipboard_service(&self) -> bool {
        self.clipboard_enabled()
            && self.peer_keyboard_enabled()
            && crate::get_builtin_option(keys::OPTION_ONE_WAY_CLIPBOARD_REDIRECTION) != "Y"
    }

    pub(super) fn audio_enabled(&self) -> bool {
        self.audio && !self.disable_audio
    }

    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    pub(super) fn file_transfer_enabled(&self) -> bool {
        self.file && self.enable_file_transfer
    }

    #[cfg(feature = "unix-file-copy-paste")]
    pub(super) fn can_sub_file_clipboard_service(&self) -> bool {
        self.clipboard_enabled()
            && self.file_transfer_enabled()
            && crate::get_builtin_option(keys::OPTION_ONE_WAY_FILE_TRANSFER) != "Y"
    }
}
