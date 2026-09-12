use super::*;

impl<T: InvokeUiSession> Session<T> {
    pub fn send_chat(&self, text: String) {
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    // Terminal methods
    pub fn open_terminal(&self, terminal_id: i32, rows: u32, cols: u32) {
        let mut action = TerminalAction::new();
        action.set_open(OpenTerminal {
            terminal_id,
            rows,
            cols,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn send_terminal_input(&self, terminal_id: i32, data: String) {
        let mut action = TerminalAction::new();
        action.set_data(TerminalData {
            terminal_id,
            data: bytes::Bytes::from(data.into_bytes()),
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn resize_terminal(&self, terminal_id: i32, rows: u32, cols: u32) {
        let mut action = TerminalAction::new();
        action.set_resize(ResizeTerminal {
            terminal_id,
            rows,
            cols,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    pub fn close_terminal(&self, terminal_id: i32) {
        let mut action = TerminalAction::new();
        action.set_close(CloseTerminal {
            terminal_id,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_terminal_action(action);
        self.send(Data::Message(msg_out));
    }

    #[inline]
    pub fn request_voice_call(&self) {
        #[cfg(target_os = "linux")]
        std::thread::spawn(crate::ipc::start_pa);
        self.send(Data::NewVoiceCall);
    }

    #[inline]
    pub fn close_voice_call(&self) {
        self.send(Data::CloseVoiceCall);
    }

    pub fn send_selected_session_id(&self, sid: String) {
        if let Ok(sid) = sid.parse::<u32>() {
            self.lc.write().unwrap().selected_windows_session_id = Some(sid);
            let mut misc = Misc::new();
            misc.set_selected_sid(sid);
            let mut msg = Message::new();
            msg.set_misc(misc);
            self.send(Data::Message(msg));
            let pi = self.lc.read().unwrap().peer_info.clone();
            if let Some(pi) = pi {
                if pi.windows_sessions.current_sid == sid {
                    if self.is_file_transfer() {
                        if pi.username.is_empty() {
                            self.on_error(
                                "No active console user logged on, please connect and logon first.",
                            );
                        } else {
                        }
                    } else if !self.is_terminal() {
                        self.msgbox(
                            "success",
                            "Successful",
                            "Connected, waiting for image...",
                            "",
                        );
                    }
                }
            }
        } else {
            log::error!("selected invalid sid: {}", sid);
        }
    }

    #[inline]
    pub fn quick_launch_request(&self, request: String) {
        if request.len() > 32768 || self.lc.read().map(|lc| lc.get_toggle_option("view-only")).unwrap_or(true) {
            return;
        }
        let mut misc = Misc::new();
        misc.set_quick_launch_request(request);
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }

    pub fn request_init_msgs(&self, display: usize) {
        self.send_message_query(display);
    }

    fn send_message_query(&self, display: usize) {
        let mut misc = Misc::new();
        misc.set_message_query(MessageQuery {
            switch_display: display as _,
            ..Default::default()
        });
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }

    pub fn get_conn_token(&self) -> Option<String> {
        self.lc.read().unwrap().get_conn_token()
    }
}
