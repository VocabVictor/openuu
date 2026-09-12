use super::*;

impl LoginConfigHandler {
    /// Create a [`Message`] for login.
    pub(super) fn create_login_msg(
        &self,
        os_username: String,
        os_password: String,
        password: Vec<u8>,
    ) -> Message {
        let my_id = Config::get_id();
        let (my_id, pure_id) = if let Some((id, _, _)) = self.other_server.as_ref() {
            let server = Config::get_rendezvous_server();
            (format!("{my_id}@{server}"), id.clone())
        } else {
            (my_id, self.id.clone())
        };
        let mut avatar = get_builtin_option(keys::OPTION_AVATAR);
        if avatar.is_empty() {
            avatar = serde_json::from_str::<serde_json::Value>(&LocalConfig::get_option(
                "user_info",
            ))
            .ok()
            .and_then(|x| {
                x.get("avatar")
                    .and_then(|x| x.as_str())
                    .map(|x| x.trim().to_owned())
            })
            .unwrap_or_default();
        }
        avatar = resolve_avatar_url(avatar);
        let mut display_name = get_builtin_option(keys::OPTION_DISPLAY_NAME);
        if display_name.is_empty() {
            display_name =
                serde_json::from_str::<serde_json::Value>(&LocalConfig::get_option("user_info"))
                    .map(|x| {
                        x.get("display_name")
                            .and_then(|x| x.as_str())
                            .map(|x| x.trim())
                            .filter(|x| !x.is_empty())
                            .or_else(|| x.get("name").and_then(|x| x.as_str()))
                            .map(|x| x.to_owned())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
        }
        if display_name.is_empty() {
            display_name = crate::username();
        }
        let display_name = display_name
            .split_whitespace()
            .map(|word| {
                word.chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_uppercase().to_string()
                        } else {
                            c.to_string()
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join(" ");
        #[cfg(not(target_os = "android"))]
        let my_platform = hbb_common::whoami::platform().to_string();
        #[cfg(target_os = "android")]
        let my_platform = "Android".into();
        let hwid = if self.get_option("trust-this-device") == "Y" {
            crate::get_hwid()
        } else {
            Bytes::new()
        };
        let os_login: MessageField<OSLogin> = if self.conn_type == ConnType::TERMINAL {
            Some(OSLogin {
                username: os_username,
                password: os_password,
                ..Default::default()
            })
            .into()
        } else {
            Default::default()
        };
        let mut lr = LoginRequest {
            username: pure_id,
            password: password.into(),
            my_id,
            my_name: display_name,
            my_platform,
            option: self.get_option_message(true).into(),
            session_id: self.session_id,
            version: crate::VERSION.to_string(),
            os_login,
            hwid,
            avatar,
            ..Default::default()
        };
        match self.conn_type {
            ConnType::FILE_TRANSFER => lr.set_file_transfer(FileTransfer {
                dir: self.get_remote_dir(),
                show_hidden: !self.get_option("remote_show_hidden").is_empty(),
                ..Default::default()
            }),
            ConnType::VIEW_CAMERA => lr.set_view_camera(Default::default()),
            ConnType::PORT_FORWARD | ConnType::RDP => lr.set_port_forward(PortForward {
                host: self.port_forward.0.clone(),
                port: self.port_forward.1,
                multiplex: self.port_forward_multiplex,
                ..Default::default()
            }),
            ConnType::TERMINAL => {
                let mut terminal = Terminal::new();
                terminal.service_id = self.get_option(self.get_key_terminal_service_id());
                lr.set_terminal(terminal);
            }
            _ => {}
        }

        let mut msg_out = Message::new();
        msg_out.set_login_request(lr);
        msg_out
    }

    pub fn update_supported_decodings(&self) -> Message {
        let decoding = scrap::codec::Decoder::supported_decodings(
            Some(&self.id),
            use_texture_render(),
            self.adapter_luid,
            &self.mark_unsupported,
        );
        let mut misc = Misc::new();
        misc.set_option(OptionMessage {
            supported_decoding: hbb_common::protobuf::MessageField::some(decoding),
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    pub fn restart_remote_device(&self) -> Message {
        let mut misc = Misc::new();
        misc.set_restart_remote_device(true);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    pub fn mark_restarting_remote_device(&mut self) {
        self.restarting_remote_device = true;
        self.restart_remote_device_at = Some(Instant::now());
    }

    pub fn clear_restarting_remote_device(&mut self) {
        self.restarting_remote_device = false;
        self.restart_remote_device_at = None;
    }

    pub fn is_restarting_remote_device(&self) -> bool {
        if !self.restarting_remote_device {
            return false;
        }
        // Keep this flag alive for a short grace window instead of clearing it on
        // connection_ready or the first peer bytes. During OS restart the peer can
        // briefly reconnect before the real reboot disconnect, and clearing it too
        // early would let the next disconnect escape the restart flow and fall back
        // to the normal error dialog / manual reconnect path.
        self.restart_remote_device_at
            .map(|started_at| started_at.elapsed() < RESTART_REMOTE_DEVICE_GRACE)
            .unwrap_or(false)
    }

    pub fn get_conn_token(&self) -> Option<String> {
        if self.password.is_empty() {
            return None;
        }
        serde_json::to_string(&ConnToken {
            password: self.password.clone(),
            password_source: self.password_source.clone(),
            session_id: self.session_id,
        })
        .ok()
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_key_terminal_service_id(&self) -> &'static str {
        if self.is_terminal_admin {
            "terminal-admin-service-id"
        } else {
            "terminal-service-id"
        }
    }
}
