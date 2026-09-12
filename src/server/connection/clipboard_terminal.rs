use super::*;

impl Connection {
    #[cfg(feature = "unix-file-copy-paste")]
    pub(super) async fn handle_file_clip(&mut self, clip: clipboard::ClipboardFile) {
        let is_stopping_allowed = clip.is_stopping_allowed();
        let file_transfer_enabled = self.file_transfer_enabled();
        let stop = is_stopping_allowed && !file_transfer_enabled;
        log::debug!(
            "Process clipboard message from clip, stop: {}, is_stopping_allowed: {}, file_transfer_enabled: {}",
            stop, is_stopping_allowed, file_transfer_enabled);
        if !stop {
            use base::config::keys::OPTION_ONE_WAY_FILE_TRANSFER;
            // Note: Code will not reach here if `crate::get_builtin_option(OPTION_ONE_WAY_FILE_TRANSFER) == "Y"` is true.
            // Because `file-clipboard` service will not be subscribed.
            // But we still check it here to keep the same logic to windows version in `ui_cm_interface.rs`.
            if clip.is_beginning_message()
                && crate::get_builtin_option(OPTION_ONE_WAY_FILE_TRANSFER) == "Y"
            {
                // If one way file transfer is enabled, don't send clipboard file to client
            } else {
                // Maybe we should end the connection, because copy&paste files causes everything to wait.
                allow_err!(
                    self.stream
                        .send(&crate::clipboard_file::clip_2_msg(clip))
                        .await
                );
            }
        }
    }

    #[inline]
    #[cfg(feature = "unix-file-copy-paste")]
    pub(super) fn try_empty_file_clipboard(&mut self) {
        try_empty_clipboard_files(ClipboardSide::Host, self.inner.id());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) async fn update_terminal_persistence(&mut self, persistent: bool) {
        self.terminal_persistent = persistent;
        terminal_service::set_persistent(&self.terminal_service_id, persistent).ok();
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) async fn init_terminal_service(&mut self) {
        debug_assert!(self.terminal_user_token.is_some());
        let Some(user_token) = self.terminal_user_token.clone() else {
            // unreachable, but keep it for safety
            log::error!("Terminal user token is not set.");
            return;
        };
        if self.terminal_service_id.is_empty() {
            self.terminal_service_id = terminal_service::generate_service_id();
        }
        let s = Box::new(terminal_service::new(
            self.terminal_service_id.clone(),
            self.terminal_persistent,
            user_token.to_terminal_service_token(),
        ));
        s.on_subscribe(self.inner.clone());
        self.terminal_generic_service = Some(s);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) async fn handle_terminal_action(&mut self, action: TerminalAction) -> ResultType<()> {
        debug_assert!(self.terminal_user_token.is_some());
        let Some(user_token) = self.terminal_user_token.clone() else {
            // unreacheable, but keep it for safety
            bail!("Terminal user token is not set.");
        };
        let mut proxy = terminal_service::TerminalServiceProxy::new(
            self.terminal_service_id.clone(),
            Some(self.terminal_persistent),
            user_token.to_terminal_service_token(),
        );

        match proxy.handle_action(&action) {
            Ok(Some(response)) => {
                let mut msg_out = Message::new();
                msg_out.set_terminal_response(response);
                self.send(msg_out).await;
            }
            Ok(None) => {
                // No response needed
            }
            Err(err) => {
                let mut response = TerminalResponse::new();
                let mut error = TerminalError::new();
                error.message = format!("Failed to handle action: {}", err);
                response.set_error(error);
                let mut msg_out = Message::new();
                msg_out.set_terminal_response(response);
                self.send(msg_out).await;
            }
        }

        Ok(())
    }
}
