use super::*;

#[async_trait]
impl<T: InvokeUiSession> Interface for Session<T> {
    fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>> {
        return self.lc.clone();
    }

    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str) {
        let direct = self.lc.read().unwrap().direct;
        let received = self.lc.read().unwrap().received;
        let retry_for_relay = direct == Some(true) && !received;
        let retry = check_if_retry(msgtype, title, text, retry_for_relay);
        self.ui_handler.msgbox(msgtype, title, text, link, retry);
    }

    fn handle_login_error(&self, err: &str) -> bool {
        handle_login_error(self.lc.clone(), err, self)
    }

    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>) {
        self.ui_handler.set_multiple_windows_session(sessions);
    }

    fn handle_peer_info(&self, mut pi: PeerInfo) {
        log::debug!("handle_peer_info :{:?}", pi);
        self.lc.write().unwrap().peer_info = Some(pi.clone());
        if pi.current_display as usize >= pi.displays.len() {
            pi.current_display = 0;
        }
        if get_version_number(&pi.version) < get_version_number("1.1.10") {
            self.set_permission("restart", false);
        }
        if self.is_file_transfer() {
            if pi.username.is_empty() && pi.windows_sessions.sessions.is_empty() {
                self.on_error("No active console user logged on, please connect and logon first.");
                return;
            }
        } else if !self.is_port_forward() && !self.is_terminal() {
            if pi.displays.is_empty() {
                self.lc.write().unwrap().handle_peer_info(&pi);
                self.update_privacy_mode();
                let msg = if self.is_view_camera() {
                    "No cameras"
                } else {
                    "No displays"
                };
                self.msgbox("error", "Error", msg, "");
                return;
            }
            if !self.is_view_camera() {
                self.try_change_init_resolution(pi.current_display);
                let p = self.lc.read().unwrap().should_auto_login();
                if !p.is_empty() {
                    input_os_password(p, true, self.clone());
                }
            }
            let current = &pi.displays[pi.current_display as usize];
            self.set_display(
                current.x,
                current.y,
                current.width,
                current.height,
                current.cursor_embedded,
                current.scale,
            );
        }
        self.update_privacy_mode();
        // Clear audit_guid when connection is established successfully
        *self.audit_guid.lock().unwrap() = String::new();
        *self.last_audit_note.lock().unwrap() = String::new();
        // Save recent peers, then push event to flutter. So flutter can refresh peer page.
        self.lc.write().unwrap().handle_peer_info(&pi);
        self.set_peer_info(&pi);
        if self.is_file_transfer() {
            self.close_success();
        } else if !self.is_port_forward() && !self.is_terminal() {
            self.msgbox(
                "success",
                "Successful",
                "Connected, waiting for image...",
                "",
            );
        }
        self.on_connected(self.lc.read().unwrap().conn_type);
        #[cfg(windows)]
        {
            let mut path = std::env::temp_dir();
            path.push(self.get_id());
            let path = path.with_extension(crate::get_app_name().to_lowercase());
            std::fs::File::create(&path).ok();
            if let Some(path) = path.to_str() {
                crate::platform::windows::add_recent_document(&path);
            }
        }
        if !pi.windows_sessions.sessions.is_empty() {
            let selected = self
                .lc
                .read()
                .unwrap()
                .selected_windows_session_id
                .to_owned();
            if selected == Some(pi.windows_sessions.current_sid) {
                self.send_selected_session_id(pi.windows_sessions.current_sid.to_string());
            } else {
                self.set_multiple_windows_session(pi.windows_sessions.sessions.clone());
            }
        }
    }

    async fn handle_hash(&self, pass: &str, hash: Hash, peer: &mut Stream) -> bool {
        handle_hash(self.lc.clone(), pass, hash, self, peer).await
    }

    async fn handle_login_from_ui(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
        peer: &mut Stream,
    ) {
        handle_login_from_ui(
            self.lc.clone(),
            os_username,
            os_password,
            password,
            remember,
            peer,
        )
        .await;
    }

    async fn handle_test_delay(&self, t: TestDelay, peer: &mut Stream) {
        if !t.from_client {
            self.update_quality_status(QualityStatus {
                delay: Some(t.last_delay as _),
                target_bitrate: Some(t.target_bitrate as _),
                ..Default::default()
            });
            handle_test_delay(t, peer).await;
        }
    }

    fn swap_modifier_mouse(&self, msg: &mut base::protos::message::MouseEvent) {
        let allow_swap_key = self.get_toggle_option("allow_swap_key".to_string());
        if allow_swap_key {
            msg.modifiers = msg
                .modifiers
                .iter()
                .map(|ck| {
                    let ck = ck.enum_value_or_default();
                    let ck = match ck {
                        ControlKey::Control => ControlKey::Meta,
                        ControlKey::Meta => ControlKey::Control,
                        ControlKey::RControl => ControlKey::Meta,
                        ControlKey::RWin => ControlKey::Control,
                        _ => ck,
                    };
                    hbb_common::protobuf::EnumOrUnknown::new(ck)
                })
                .collect();
        };
    }
}
