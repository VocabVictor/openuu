use super::*;

impl Connection {
    pub(super) fn is_supported_decoding_only_option(option: &OptionMessage) -> bool {
        option.supported_decoding.is_some()
            && option.image_quality.enum_value() == Ok(ImageQuality::NotSet)
            && option.custom_image_quality == 0
            && option.custom_fps == 0
            && Self::is_bool_option_not_set(option.lock_after_session_end)
            && Self::is_bool_option_not_set(option.show_remote_cursor)
            && Self::is_bool_option_not_set(option.privacy_mode)
            && Self::is_bool_option_not_set(option.block_input)
            && Self::is_bool_option_not_set(option.disable_audio)
            && Self::is_bool_option_not_set(option.disable_clipboard)
            && Self::is_bool_option_not_set(option.enable_file_transfer)
            && Self::is_bool_option_not_set(option.disable_keyboard)
            && Self::is_bool_option_not_set(option.follow_remote_cursor)
            && Self::is_bool_option_not_set(option.follow_remote_window)
            && Self::is_bool_option_not_set(option.disable_camera)
            && Self::is_bool_option_not_set(option.terminal_persistent)
            && Self::is_bool_option_not_set(option.show_my_cursor)
    }

    pub(super) fn is_connection_housekeeping_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::LoginRequest(_)) => true,
            Some(message::Union::TestDelay(_)) => true,
            Some(message::Union::Misc(misc)) => {
                matches!(misc.union.as_ref(), Some(misc::Union::CloseReason(_)))
            }
            _ => false,
        }
    }

    pub(super) fn is_file_transfer_scoped_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::FileAction(_)) | Some(message::Union::FileResponse(_)) => true,
            Some(message::Union::Misc(misc)) => Self::is_file_transfer_scoped_misc(misc),
            _ => false,
        }
    }

    pub(super) fn is_file_transfer_scoped_misc(misc: &Misc) -> bool {
        #[cfg(windows)]
        if matches!(misc.union.as_ref(), Some(misc::Union::SelectedSid(_))) {
            return true;
        }
        #[cfg(not(windows))]
        let _ = misc;
        false
    }

    pub(super) fn is_port_forward_scoped_message(msg: &Message) -> bool {
        matches!(
            msg.union.as_ref(),
            Some(message::Union::PortForwardChannel(_))
        )
    }

    pub(super) fn is_terminal_scoped_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::TerminalAction(_)) => true,
            Some(message::Union::Misc(misc)) => Self::is_terminal_scoped_misc(misc),
            _ => false,
        }
    }

    pub(super) fn is_terminal_scoped_misc(misc: &Misc) -> bool {
        match misc.union.as_ref() {
            Some(misc::Union::ChatMessage(_)) => true,
            Some(misc::Union::Option(option)) => Self::is_terminal_scoped_option(option),
            _ => false,
        }
    }

    pub(super) fn is_terminal_scoped_option(option: &OptionMessage) -> bool {
        Self::scoped_terminal_login_option(option).1.is_none()
    }

    pub(super) fn is_view_camera_scoped_message(msg: &Message) -> bool {
        match msg.union.as_ref() {
            Some(message::Union::ScreenshotRequest(_)) => true,
            Some(message::Union::Misc(misc)) => Self::is_view_camera_scoped_misc(misc),
            // Legacy clients may send auto-login input during view-camera connect.
            // The handlers intentionally ignore these messages for view-camera sessions.
            Some(message::Union::MouseEvent(_))
            | Some(message::Union::PointerDeviceEvent(_))
            | Some(message::Union::KeyEvent(_)) => true,
            Some(message::Union::AudioFrame(_))
            | Some(message::Union::VoiceCallRequest(_))
            | Some(message::Union::VoiceCallResponse(_)) => true,
            _ => false,
        }
    }

    pub(super) fn is_view_camera_scoped_misc(misc: &Misc) -> bool {
        match misc.union.as_ref() {
            Some(misc::Union::SwitchDisplay(_))
            | Some(misc::Union::CaptureDisplays(_))
            | Some(misc::Union::RefreshVideo(_))
            | Some(misc::Union::RefreshVideoDisplay(_))
            | Some(misc::Union::VideoReceived(_))
            | Some(misc::Union::ChatMessage(_))
            | Some(misc::Union::AudioFormat(_))
            | Some(misc::Union::ClientRecordStatus(_))
            // Though these messages are not expected in normal view-camera sessions,
            // keep them allowed to avoid breaking existing clients that may send them.
            | Some(misc::Union::MessageQuery(_))
            | Some(misc::Union::TogglePrivacyMode(_))
            | Some(misc::Union::ToggleVirtualDisplay(_))
            | Some(misc::Union::ChangeResolution(_))
            | Some(misc::Union::ChangeDisplayResolution(_)) => true,
            Some(misc::Union::Option(option)) => Self::is_view_camera_scoped_option(option),
            #[cfg(windows)]
            Some(misc::Union::SelectedSid(_)) => true,
            _ => false,
        }
    }

    pub(super) fn is_view_camera_scoped_option(option: &OptionMessage) -> bool {
        Self::scoped_view_camera_option(option).1.is_none()
    }

    // Keep these OptionMessage field lists in sync with message.proto and update_options().
    // New fields must be classified here before limited session types can receive them.
    pub(super) fn scoped_view_camera_option(
        option: &OptionMessage,
    ) -> (Option<OptionMessage>, Option<&'static str>) {
        let mut scoped = OptionMessage::new();
        let mut violation = false;
        if option.image_quality.enum_value().is_ok() {
            scoped.image_quality = option.image_quality;
        }
        if option.custom_image_quality >= 0 {
            scoped.custom_image_quality = option.custom_image_quality;
        }
        if option.custom_fps >= 0 {
            scoped.custom_fps = option.custom_fps;
        }
        scoped.supported_decoding = option.supported_decoding.clone();
        if let Ok(value) = option.disable_audio.enum_value() {
            scoped.disable_audio = value.into();
        }
        if Self::option_has_non_view_camera_login_field(option) {
            violation = true;
        }
        let scoped = Self::option_has_any_field(&scoped).then_some(scoped);
        (scoped, violation.then_some("login.option"))
    }

    pub(super) fn option_has_non_view_camera_login_field(option: &OptionMessage) -> bool {
        !(Self::is_bool_option_not_set(option.lock_after_session_end)
            && Self::is_bool_option_not_set(option.show_remote_cursor)
            && Self::is_bool_option_not_set(option.privacy_mode)
            && Self::is_bool_option_not_set(option.block_input)
            && Self::is_bool_option_not_set(option.disable_clipboard)
            && Self::is_bool_option_not_set(option.enable_file_transfer)
            && Self::is_bool_option_not_set(option.disable_keyboard)
            && Self::is_bool_option_not_set(option.follow_remote_cursor)
            && Self::is_bool_option_not_set(option.follow_remote_window)
            && Self::is_bool_option_not_set(option.disable_camera)
            && Self::is_bool_option_not_set(option.terminal_persistent)
            && Self::is_bool_option_not_set(option.show_my_cursor))
    }

    pub(super) fn option_has_non_terminal_login_field(option: &OptionMessage) -> bool {
        option.image_quality.enum_value() != Ok(ImageQuality::NotSet)
            || option.custom_image_quality != 0
            || option.custom_fps != 0
            || option.supported_decoding.is_some()
            || !Self::is_bool_option_not_set(option.lock_after_session_end)
            || !Self::is_bool_option_not_set(option.show_remote_cursor)
            || !Self::is_bool_option_not_set(option.privacy_mode)
            || !Self::is_bool_option_not_set(option.block_input)
            || !Self::is_bool_option_not_set(option.disable_audio)
            || !Self::is_bool_option_not_set(option.disable_clipboard)
            || !Self::is_bool_option_not_set(option.enable_file_transfer)
            || !Self::is_bool_option_not_set(option.disable_keyboard)
            || !Self::is_bool_option_not_set(option.follow_remote_cursor)
            || !Self::is_bool_option_not_set(option.follow_remote_window)
            || !Self::is_bool_option_not_set(option.disable_camera)
            || !Self::is_bool_option_not_set(option.show_my_cursor)
    }

    pub(super) fn option_has_any_field(option: &OptionMessage) -> bool {
        Self::option_has_non_terminal_login_field(option)
            || !Self::is_bool_option_not_set(option.terminal_persistent)
    }

    pub(super) fn is_bool_option_not_set(option: hbb_common::protobuf::EnumOrUnknown<BoolOption>) -> bool {
        option.enum_value() == Ok(BoolOption::NotSet)
    }

    pub(super) fn message_family(msg: &Message) -> &'static str {
        match msg.union.as_ref() {
            Some(message::Union::MouseEvent(_)) => "mouse_event",
            Some(message::Union::AudioFrame(_)) => "audio_frame",
            Some(message::Union::PointerDeviceEvent(_)) => "pointer_device_event",
            Some(message::Union::KeyEvent(_)) => "key_event",
            Some(message::Union::Clipboard(_)) => "clipboard",
            Some(message::Union::FileAction(_)) => "file_action",
            Some(message::Union::FileResponse(_)) => "file_response",
            Some(message::Union::VoiceCallRequest(_)) => "voice_call_request",
            Some(message::Union::VoiceCallResponse(_)) => "voice_call_response",
            Some(message::Union::MultiClipboards(_)) => "multi_clipboards",
            Some(message::Union::ScreenshotRequest(_)) => "screenshot_request",
            Some(message::Union::ScreenshotResponse(_)) => "screenshot_response",
            Some(message::Union::TerminalAction(_)) => "terminal_action",
            Some(message::Union::TerminalResponse(_)) => "terminal_response",
            Some(message::Union::PortForwardChannel(_)) => "port_forward_channel",
            Some(message::Union::Misc(misc)) => Self::misc_message_family(misc),
            Some(_) => "message.other",
            None => "empty",
        }
    }

    pub(super) fn misc_message_family(misc: &Misc) -> &'static str {
        match misc.union.as_ref() {
            Some(misc::Union::ChatMessage(_)) => "misc.chat_message",
            Some(misc::Union::SwitchDisplay(_)) => "misc.switch_display",
            Some(misc::Union::Option(_)) => "misc.option",
            Some(misc::Union::AudioFormat(_)) => "misc.audio_format",
            Some(misc::Union::CaptureDisplays(_)) => "misc.capture_displays",
            Some(misc::Union::ClientRecordStatus(_)) => "misc.client_record_status",
            Some(misc::Union::TogglePrivacyMode(_)) => "misc.toggle_privacy_mode",
            Some(misc::Union::ToggleVirtualDisplay(_)) => "misc.toggle_virtual_display",
            Some(misc::Union::SelectedSid(_)) => "misc.selected_sid",
            Some(misc::Union::ChangeResolution(_)) => "misc.change_resolution",
            Some(misc::Union::ChangeDisplayResolution(_)) => "misc.change_display_resolution",
            Some(misc::Union::MessageQuery(_)) => "misc.message_query",
            Some(misc::Union::FollowCurrentDisplay(_)) => "misc.follow_current_display",
            Some(misc::Union::SwitchSidesRequest(_)) => "misc.switch_sides_request",
            Some(_) => "misc.other",
            None => "misc.empty",
        }
    }
}
