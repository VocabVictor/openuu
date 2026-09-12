use super::*;

impl LoginConfigHandler {
    /// Get [`PeerConfig`] of the current [`LoginConfigHandler`].
    ///
    /// # Arguments
    pub fn get_config(&mut self) -> &mut PeerConfig {
        &mut self.config
    }

    /// Get [`OptionMessage`] of the current [`LoginConfigHandler`].
    /// Return `None` if there's no option, for example, when the session is only for file transfer.
    ///
    /// # Arguments
    ///
    /// * `ignore_default` - If `true`, ignore the default value of the option.
    pub(super) fn get_option_message(&self, ignore_default: bool) -> Option<OptionMessage> {
        if self.conn_type.eq(&ConnType::PORT_FORWARD)
            || self.conn_type.eq(&ConnType::RDP)
            || self.conn_type.eq(&ConnType::FILE_TRANSFER)
        {
            return None;
        }
        let mut msg = OptionMessage::new();
        if self.conn_type.eq(&ConnType::TERMINAL) {
            if self.get_toggle_option(keys::OPTION_TERMINAL_PERSISTENT) {
                msg.terminal_persistent = BoolOption::Yes.into();
                return Some(msg);
            } else {
                return None;
            }
        }
        let q = self.image_quality.clone();
        if let Some(q) = self.get_image_quality_enum(&q, ignore_default) {
            msg.image_quality = q.into();
        } else if q == "custom" {
            let config = self.load_config();
            let allow_more = !crate::using_public_server() || self.direct == Some(true);
            let quality = if config.custom_image_quality.is_empty() {
                50
            } else {
                let mut quality = config.custom_image_quality[0];
                if !allow_more && quality > 100 {
                    quality = 50;
                }
                quality
            };
            msg.custom_image_quality = quality << 8;
            #[cfg(feature = "flutter")]
            if let Some(custom_fps) = self.options.get("custom-fps") {
                let mut custom_fps = custom_fps.parse().unwrap_or(30);
                if !allow_more && custom_fps > 30 {
                    custom_fps = 30;
                }
                msg.custom_fps = custom_fps;
                *self.custom_fps.lock().unwrap() = Some(custom_fps as _);
            }
        }
        let view_only = self.get_toggle_option("view-only");
        if view_only {
            msg.disable_keyboard = BoolOption::Yes.into();
            msg.enable_file_transfer = BoolOption::No.into();
            msg.lock_after_session_end = BoolOption::No.into();
        }
        if view_only || self.get_toggle_option("show-remote-cursor") {
            msg.show_remote_cursor = BoolOption::Yes.into();
        }
        if view_only && self.get_toggle_option("show-my-cursor") {
            msg.show_my_cursor = BoolOption::Yes.into();
        }
        if self.get_toggle_option("follow-remote-cursor") {
            msg.follow_remote_cursor = BoolOption::Yes.into();
        }
        if self.get_toggle_option("follow-remote-window") {
            msg.follow_remote_window = BoolOption::Yes.into();
        }
        if !view_only && self.get_toggle_option("lock-after-session-end") {
            msg.lock_after_session_end = BoolOption::Yes.into();
        }
        if self.get_toggle_option("disable-audio") {
            msg.disable_audio = BoolOption::Yes.into();
        }
        if !view_only && self.get_toggle_option(keys::OPTION_ENABLE_FILE_COPY_PASTE) {
            msg.enable_file_transfer = BoolOption::Yes.into();
        }
        if view_only || self.get_toggle_option("disable-clipboard") {
            msg.disable_clipboard = BoolOption::Yes.into();
        }
        msg.supported_decoding = MessageField::some(self.get_supported_decoding());
        Some(msg)
    }

    pub fn get_supported_decoding(&self) -> SupportedDecoding {
        Decoder::supported_decodings(
            Some(&self.id),
            use_texture_render(),
            self.adapter_luid,
            &self.mark_unsupported,
        )
    }

    /// Parse the image quality option.
    /// Return [`ImageQuality`] if the option is valid, otherwise return `None`.
    ///
    /// # Arguments
    ///
    /// * `q` - The image quality option.
    /// * `ignore_default` - Ignore the default value.
    pub(super) fn get_image_quality_enum(&self, q: &str, ignore_default: bool) -> Option<ImageQuality> {
        if q == "low" {
            Some(ImageQuality::Low)
        } else if q == "best" {
            Some(ImageQuality::Best)
        } else if q == "balanced" {
            if ignore_default {
                None
            } else {
                Some(ImageQuality::Balanced)
            }
        } else {
            None
        }
    }

    /// Get the status of a toggle option.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the toggle option.
    ///
    // It's Ok to check the option empty in this function.
    // `get_toggle_option()` is only called in a session.
    // Custom client advanced settings will not effect this function.
    pub fn get_toggle_option(&self, name: &str) -> bool {
        if name == "show-remote-cursor" {
            self.config.show_remote_cursor.v
        } else if name == "lock-after-session-end" {
            self.config.lock_after_session_end.v
        } else if name == keys::OPTION_TERMINAL_PERSISTENT {
            self.config.terminal_persistent.v
        } else if name == "privacy-mode" {
            self.config.privacy_mode.v
        } else if name == keys::OPTION_ENABLE_FILE_COPY_PASTE {
            self.config.enable_file_copy_paste.v
        } else if name == "disable-audio" {
            self.config.disable_audio.v
        } else if name == "disable-clipboard" {
            self.config.disable_clipboard.v
        } else if name == "show-quality-monitor" {
            self.config.show_quality_monitor.v
        } else if name == "allow_swap_key" {
            self.config.allow_swap_key.v
        } else if name == "view-only-session" {
            self.view_only_session
        } else if name == "view-only" {
            self.view_only_session || self.config.view_only.v
        } else if name == "show-my-cursor" {
            self.config.show_my_cursor.v
        } else if name == "follow-remote-cursor" {
            self.config.follow_remote_cursor.v
        } else if name == "follow-remote-window" {
            self.config.follow_remote_window.v
        } else {
            !self.get_option(name).is_empty()
        }
    }

    pub fn is_privacy_mode_supported(&self) -> bool {
        if let Some(features) = &self.features {
            features.privacy_mode
        } else {
            false
        }
    }

    /// Create a [`Message`] for refreshing video.
    pub fn refresh() -> Message {
        let mut misc = Misc::new();
        misc.set_refresh_video(true);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }

    /// Create a [`Message`] for refreshing video.
    pub fn refresh_display(display: usize) -> Message {
        let mut misc = Misc::new();
        misc.set_refresh_video_display(display as _);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        msg_out
    }
}
