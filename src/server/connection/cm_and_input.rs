use super::*;

impl Connection {
    pub(super) fn try_start_cm(&mut self, peer_id: String, name: String, authorized: bool) {
        self.send_to_cm(ipc::Data::Login {
            id: self.inner.id(),
            is_file_transfer: self.file_transfer.is_some(),
            is_view_camera: self.view_camera,
            is_terminal: self.terminal,
            port_forward: self.port_forward_address.clone(),
            peer_id,
            name,
            avatar: self.lr.avatar.clone(),
            authorized,
            keyboard: self.keyboard,
            clipboard: self.clipboard,
            audio: self.audio,
            file: self.file,
            file_transfer_enabled: self.file,
            restart: self.restart,
            recording: self.recording,
            block_input: self.block_input,
            privacy_mode: self.privacy_mode,
            from_switch: self.from_switch,
        });
    }

    #[inline]
    pub(super) fn send_to_cm(&mut self, data: ipc::Data) {
        self.tx_to_cm.send(data).ok();
    }

    pub(super) fn handle_port_forward_channel(&mut self, ch: PortForwardChannel) {
        let Some(mux) = self.port_forward_mux.as_mut() else {
            log::debug!("port forward channel frame on a non-multiplexed connection");
            return;
        };
        mux.handle(ch, || {
            Self::permission(keys::OPTION_ENABLE_TUNNEL, &self.control_permissions)
        });
    }

    #[inline]
    pub(super) fn send_fs(&mut self, data: ipc::FS) {
        self.send_to_cm(ipc::Data::FS(data));
    }

    pub(super) async fn send_login_error<T: std::string::ToString>(&mut self, err: T) {
        let mut msg_out = Message::new();
        let mut res = LoginResponse::new();
        res.set_error(err.to_string());
        if err.to_string() == crate::client::REQUIRE_2FA {
            res.enable_trusted_devices = Self::enable_trusted_devices();
        }
        msg_out.set_login_response(res);
        self.send(msg_out).await;
    }

    #[inline]
    pub fn send_block_input_error(
        s: &Sender,
        state: back_notification::BlockInputState,
        details: String,
    ) {
        let mut misc = Misc::new();
        let mut back_notification = BackNotification {
            details,
            ..Default::default()
        };
        back_notification.set_block_input_state(state);
        misc.set_back_notification(back_notification);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        s.send((Instant::now(), Arc::new(msg_out))).ok();
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn input_mouse(
        &self,
        msg: MouseEvent,
        conn_id: i32,
        username: String,
        argb: u32,
        simulate: bool,
        show_cursor: bool,
    ) {
        self.tx_input
            .send(MessageInput::Mouse(InputMouse {
                msg,
                conn_id,
                username,
                argb,
                simulate,
                show_cursor,
            }))
            .ok();
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn input_pointer(&self, msg: PointerDeviceEvent, conn_id: i32) {
        self.tx_input
            .send(MessageInput::Pointer((msg, conn_id)))
            .ok();
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn input_key(&self, msg: KeyEvent, press: bool) {
        // to-do: if is the legacy mode, and the key is function key "LockScreen".
        // Switch to the primary display.
        self.tx_input.send(MessageInput::Key((msg, press))).ok();
    }
}
