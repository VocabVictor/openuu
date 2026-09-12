use super::*;

impl Connection {
    #[inline]
    pub(super) fn session_key(&self) -> SessionKey {
        SessionKey {
            peer_id: self.lr.my_id.clone(),
            name: self.lr.my_name.clone(),
            session_id: self.lr.session_id,
        }
    }

    pub(super) fn is_authed_remote_conn(&self) -> bool {
        if let Some(id) = self.authed_conn_id.as_ref() {
            return id.conn_type() == AuthConnType::Remote;
        }
        false
    }

    pub(super) fn is_authed_view_camera_conn(&self) -> bool {
        if let Some(id) = self.authed_conn_id.as_ref() {
            return id.conn_type() == AuthConnType::ViewCamera;
        }
        false
    }

    pub(super) fn should_handle_render_broadcast_message(&self) -> bool {
        matches!(
            self.authed_conn_type(),
            Some(AuthConnType::Remote | AuthConnType::ViewCamera)
        )
    }

    pub(super) fn should_handle_text_clipboard_message(&self) -> bool {
        matches!(self.authed_conn_type(), Some(AuthConnType::Remote))
    }

    pub(super) fn scoped_update_option_message(&self, option: &OptionMessage) -> Option<OptionMessage> {
        match self.authed_conn_type() {
            Some(AuthConnType::ViewCamera) => Self::scoped_view_camera_option(option).0,
            Some(AuthConnType::Terminal) => Self::scoped_terminal_login_option(option).0,
            Some(AuthConnType::Remote | AuthConnType::FileTransfer | AuthConnType::PortForward)
            | None => None,
        }
    }

    pub(super) fn authed_conn_type(&self) -> Option<AuthConnType> {
        self.authed_conn_id.as_ref().map(|id| id.conn_type())
    }

    pub(super) async fn handle_authorized_scope_violation(&mut self, message: &'static str) -> bool {
        let conn_type = self
            .authed_conn_type()
            .map(AuthConnType::as_str)
            .unwrap_or("unknown");
        let is_first = self.scope_violation_messages.insert(message);
        if is_first {
            log::warn!(
                "Received out-of-scope message in {} session: {}",
                conn_type,
                message
            );
        } else {
            log::debug!(
                "Received repeated out-of-scope message in {} session: {}",
                conn_type,
                message
            );
        }
        if is_first && Config::get_bool_option(keys::OPTION_ALLOW_SCOPE_VIOLATION_ALARM) {
            self.post_session_scope_violation_alarm(message);
        }
        if Config::get_bool_option(keys::OPTION_ALLOW_SCOPE_VIOLATION_CLOSE) {
            self.send_close_reason_no_retry("Connection not allowed")
                .await;
            self.on_close("Session scope violation", true).await;
            return false;
        }
        true
    }

    pub(super) fn authorized_scope_violation(&self, msg: &Message) -> Option<&'static str> {
        let Some(conn_type) = self.authed_conn_type() else {
            return (!Self::is_connection_housekeeping_message(msg)).then_some("session.auth_type");
        };
        Self::authorized_message_scope_violation(conn_type, msg)
    }

    pub(super) async fn update_scoped_login_options(&mut self) {
        let Some(option) = self.options_in_login.take() else {
            return;
        };
        let Some(conn_type) = self.authed_conn_type() else {
            // Unreachable, but just in case, we drop the options if the connection type is unknown.
            log::warn!(
                "Dropping scoped login options because authorized connection type is unknown"
            );
            return;
        };
        let (scoped, violation) = Self::scoped_login_option(conn_type, &option);
        if let Some(message) = violation {
            log::debug!(
                "Filtering {} session login options outside scope: {}",
                conn_type.as_str(),
                message
            );
        }
        if let Some(option) = scoped {
            self.update_options(&option).await;
        }
    }

    pub(super) fn scoped_login_option(
        conn_type: AuthConnType,
        option: &OptionMessage,
    ) -> (Option<OptionMessage>, Option<&'static str>) {
        match conn_type {
            AuthConnType::Remote => (Some(option.clone()), None),
            AuthConnType::ViewCamera => Self::scoped_view_camera_option(option),
            AuthConnType::Terminal => Self::scoped_terminal_login_option(option),
            AuthConnType::FileTransfer | AuthConnType::PortForward => {
                let violation = Self::option_has_any_field(option).then_some("login.option");
                (None, violation)
            }
        }
    }

    pub(super) fn scoped_terminal_login_option(
        option: &OptionMessage,
    ) -> (Option<OptionMessage>, Option<&'static str>) {
        let mut scoped = OptionMessage::new();
        let mut violation = false;
        match option.terminal_persistent.enum_value() {
            Ok(value) => scoped.terminal_persistent = value.into(),
            Err(_) => violation = true,
        }
        if Self::option_has_non_terminal_login_field(option) {
            violation = true;
        }
        let scoped = Self::option_has_any_field(&scoped).then_some(scoped);
        (scoped, violation.then_some("login.option"))
    }

    pub(super) fn authorized_message_scope_violation(
        conn_type: AuthConnType,
        msg: &Message,
    ) -> Option<&'static str> {
        if Self::is_connection_housekeeping_message(msg) {
            return None;
        }
        // Legacy clients can broadcast render-refresh messages to all opened sessions.
        // Clipboard messages may also be broadcast to FileTransfer/Terminal sessions while
        // the client still considers text clipboard sync required, and handlers ignore them.
        let noop_compat = match conn_type {
            AuthConnType::FileTransfer | AuthConnType::Terminal => {
                Self::is_render_broadcast_noop_compat_message(msg)
                    || Self::is_text_clipboard_noop_compat_message(msg)
            }
            AuthConnType::PortForward => Self::is_render_broadcast_noop_compat_message(msg),
            AuthConnType::ViewCamera => Self::is_text_clipboard_noop_compat_message(msg),
            _ => false,
        };
        if noop_compat {
            return None;
        }
        let allowed = match conn_type {
            AuthConnType::Remote => true,
            AuthConnType::FileTransfer => Self::is_file_transfer_scoped_message(msg),
            AuthConnType::PortForward => Self::is_port_forward_scoped_message(msg),
            AuthConnType::ViewCamera => Self::is_view_camera_scoped_message(msg),
            AuthConnType::Terminal => Self::is_terminal_scoped_message(msg),
        };
        (!allowed).then(|| Self::message_family(msg))
    }

    pub(super) fn is_render_broadcast_noop_compat_message(msg: &Message) -> bool {
        let Some(message::Union::Misc(misc)) = msg.union.as_ref() else {
            return false;
        };
        match misc.union.as_ref() {
            Some(misc::Union::RefreshVideo(_)) | Some(misc::Union::RefreshVideoDisplay(_)) => true,
            Some(misc::Union::Option(option)) => Self::is_supported_decoding_only_option(option),
            _ => false,
        }
    }

    pub(super) fn is_text_clipboard_noop_compat_message(msg: &Message) -> bool {
        matches!(
            msg.union.as_ref(),
            Some(message::Union::Clipboard(_)) | Some(message::Union::MultiClipboards(_))
        )
    }
}
