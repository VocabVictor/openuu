use super::*;

#[test]
pub(super) fn session_scope_allows_only_messages_for_authenticated_session_type() {
    let cases = [
        (
            AuthConnType::FileTransfer,
            vec![
                (msg(|m| m.set_file_action(FileAction::new())), None),
                (msg(|m| m.set_file_response(FileResponse::new())), None),
                (msg(|m| m.set_login_request(LoginRequest::new())), None),
                (
                    msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                    Some("screenshot_request"),
                ),
                (
                    misc_msg(|m| m.set_capture_displays(CaptureDisplays::new())),
                    Some("misc.capture_displays"),
                ),
                (
                    misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                    Some("misc.switch_sides_request"),
                ),
                (msg(|m| m.set_clipboard(Clipboard::new())), None),
                (
                    msg(|m| m.set_multi_clipboards(MultiClipboards::new())),
                    None,
                ),
                (misc_msg(|m| m.set_refresh_video(true)), None),
                (misc_msg(|m| m.set_refresh_video_display(0)), None),
                (
                    option_msg(|o| {
                        o.supported_decoding =
                            hbb_common::protobuf::MessageField::some(Default::default())
                    }),
                    None,
                ),
                (
                    option_msg(|o| {
                        o.supported_decoding =
                            hbb_common::protobuf::MessageField::some(Default::default());
                        o.disable_audio = BoolOption::Yes.into();
                    }),
                    Some("misc.option"),
                ),
                (
                    msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                    Some("port_forward_channel"),
                ),
            ],
        ),
        (
            AuthConnType::Terminal,
            vec![
                (msg(|m| m.set_terminal_action(TerminalAction::new())), None),
                (
                    option_msg(|o| o.terminal_persistent = BoolOption::Yes.into()),
                    None,
                ),
                (
                    msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                    Some("screenshot_request"),
                ),
                (
                    msg(|m| m.set_file_action(FileAction::new())),
                    Some("file_action"),
                ),
                (
                    misc_msg(|m| m.set_toggle_privacy_mode(TogglePrivacyMode::new())),
                    Some("misc.toggle_privacy_mode"),
                ),
                (
                    misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                    Some("misc.switch_sides_request"),
                ),
                (misc_msg(|m| m.set_chat_message(ChatMessage::new())), None),
                (msg(|m| m.set_clipboard(Clipboard::new())), None),
                (
                    msg(|m| m.set_multi_clipboards(MultiClipboards::new())),
                    None,
                ),
                (
                    misc_msg(|m| m.set_toggle_virtual_display(ToggleVirtualDisplay::new())),
                    Some("misc.toggle_virtual_display"),
                ),
                (
                    misc_msg(|m| m.set_change_resolution(Resolution::new())),
                    Some("misc.change_resolution"),
                ),
                (
                    misc_msg(|m| m.set_change_display_resolution(DisplayResolution::new())),
                    Some("misc.change_display_resolution"),
                ),
                (misc_msg(|m| m.set_refresh_video(true)), None),
                (misc_msg(|m| m.set_refresh_video_display(0)), None),
                (
                    option_msg(|o| {
                        o.supported_decoding =
                            hbb_common::protobuf::MessageField::some(Default::default())
                    }),
                    None,
                ),
                (
                    option_msg(|o| {
                        o.supported_decoding =
                            hbb_common::protobuf::MessageField::some(Default::default());
                        o.disable_audio = BoolOption::Yes.into();
                    }),
                    Some("misc.option"),
                ),
                (
                    msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                    Some("port_forward_channel"),
                ),
            ],
        ),
        (
            AuthConnType::ViewCamera,
            vec![
                (
                    misc_msg(|m| m.set_switch_display(SwitchDisplay::new())),
                    None,
                ),
                (misc_msg(|m| m.set_chat_message(ChatMessage::new())), None),
                (
                    msg(|m| m.set_voice_call_request(VoiceCallRequest::new())),
                    None,
                ),
                (msg(|m| m.set_audio_frame(AudioFrame::new())), None),
                (
                    option_msg(|o| o.image_quality = ImageQuality::Balanced.into()),
                    None,
                ),
                (
                    misc_msg(|m| m.set_toggle_privacy_mode(TogglePrivacyMode::new())),
                    None,
                ),
                (
                    misc_msg(|m| m.set_toggle_virtual_display(ToggleVirtualDisplay::new())),
                    None,
                ),
                (
                    misc_msg(|m| m.set_change_resolution(Resolution::new())),
                    None,
                ),
                (
                    misc_msg(|m| m.set_change_display_resolution(DisplayResolution::new())),
                    None,
                ),
                (msg(|m| m.set_mouse_event(MouseEvent::new())), None),
                (
                    msg(|m| m.set_pointer_device_event(PointerDeviceEvent::new())),
                    None,
                ),
                (msg(|m| m.set_key_event(KeyEvent::new())), None),
                (misc_msg(|m| m.set_client_record_status(true)), None),
                (
                    msg(|m| m.set_file_response(FileResponse::new())),
                    Some("file_response"),
                ),
                (
                    msg(|m| m.set_terminal_action(TerminalAction::new())),
                    Some("terminal_action"),
                ),
                (
                    misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                    Some("misc.switch_sides_request"),
                ),
            ],
        ),
        (
            AuthConnType::Remote,
            vec![
                (
                    msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                    None,
                ),
                (msg(|m| m.set_terminal_action(TerminalAction::new())), None),
                (
                    misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                    None,
                ),
            ],
        ),
        (
            AuthConnType::PortForward,
            vec![
                (msg(|m| m.set_test_delay(TestDelay::new())), None),
                (misc_msg(|m| m.set_close_reason("closed".to_owned())), None),
                (
                    msg(|m| m.set_file_action(FileAction::new())),
                    Some("file_action"),
                ),
                (
                    msg(|m| m.set_terminal_action(TerminalAction::new())),
                    Some("terminal_action"),
                ),
                (
                    msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                    Some("screenshot_request"),
                ),
                (
                    misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                    Some("misc.switch_sides_request"),
                ),
                (misc_msg(|m| m.set_refresh_video(true)), None),
                (misc_msg(|m| m.set_refresh_video_display(0)), None),
                (
                    option_msg(|o| {
                        o.supported_decoding =
                            hbb_common::protobuf::MessageField::some(Default::default())
                    }),
                    None,
                ),
                (
                    msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                    None,
                ),
            ],
        ),
    ];

    for (conn_type, messages) in cases {
        assert_scopes(conn_type, messages);
    }
}
