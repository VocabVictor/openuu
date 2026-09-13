use super::*;

#[tokio::test]
async fn cursor_messages_reach_the_ui() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut cd = Message::new();
    cd.set_cursor_data(CursorData {
        id: 7,
        ..Default::default()
    });
    let mut cp = Message::new();
    cp.set_cursor_position(CursorPosition {
        x: 3,
        y: 4,
        ..Default::default()
    });
    assert!(feed(&mut parts, &cd).await);
    assert!(feed(&mut parts, &cp).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["set_cursor_data:7", "set_cursor_position:3,4"]
    );
}

#[tokio::test]
async fn a_message_box_keeps_only_whitelisted_links() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut known = Message::new();
    known.set_message_box(MessageBox {
        msgtype: "info".to_owned(),
        title: "T".to_owned(),
        text: "hello".to_owned(),
        link: "rustdesk docs home".to_owned(),
        ..Default::default()
    });
    let mut unknown = Message::new();
    unknown.set_message_box(MessageBox {
        msgtype: "info".to_owned(),
        title: "T".to_owned(),
        text: "hello".to_owned(),
        link: "https://evil.example/".to_owned(),
        ..Default::default()
    });
    assert!(feed(&mut parts, &known).await);
    assert!(feed(&mut parts, &unknown).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec![
            format!("msgbox:info|T|hello|{}|false", config::LINK_DOCS_HOME),
            "msgbox:info|T|hello||false".to_owned(),
        ]
    );
}

#[tokio::test]
async fn a_bare_peer_info_updates_displays_and_platform_additions() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut msg = Message::new();
    msg.set_peer_info(PeerInfo {
        displays: vec![DisplayInfo::default(), DisplayInfo::default()],
        platform_additions: "{}".to_owned(),
        ..Default::default()
    });
    assert!(feed(&mut parts, &msg).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["set_displays:2", "set_platform_additions:{}"]
    );
}

#[tokio::test]
async fn a_test_delay_from_the_peer_is_echoed_and_reported() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut msg = Message::new();
    msg.set_test_delay(TestDelay {
        time: 11,
        from_client: false,
        last_delay: 20,
        target_bitrate: 300,
        ..Default::default()
    });
    assert!(feed(&mut parts, &msg).await);
    let echo = next_message(&mut parts.far_end).await;
    assert_eq!(echo.test_delay().time, 11);
    assert!(parts.remote.handler.has_call("update_quality_status"));

    let mut own = Message::new();
    own.set_test_delay(TestDelay {
        time: 12,
        from_client: true,
        ..Default::default()
    });
    assert!(feed(&mut parts, &own).await);
    assert!(
        try_next_message(&mut parts.far_end, 200).await.is_none(),
        "our own probe must not be echoed"
    );
}

#[tokio::test]
async fn audio_frames_follow_the_disable_audio_option() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut msg = Message::new();
    msg.set_audio_frame(AudioFrame::default());
    assert!(feed(&mut parts, &msg).await);
    assert!(matches!(
        parts.rx_media.try_recv(),
        Ok(MediaData::AudioFrame(_))
    ));

    parts.remote.handler.lc.write().unwrap().config.disable_audio.v = true;
    assert!(feed(&mut parts, &msg).await);
    assert!(parts.rx_media.try_recv().is_err());
}

#[tokio::test]
async fn screenshot_and_terminal_responses_reach_the_ui() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut shot = Message::new();
    shot.set_screenshot_response(ScreenshotResponse {
        sid: "s1".to_owned(),
        msg: "ok".to_owned(),
        ..Default::default()
    });
    let mut term = Message::new();
    term.set_terminal_response(TerminalResponse {
        union: Some(terminal_response::Union::Opened(TerminalOpened {
            success: false,
            ..Default::default()
        })),
        ..Default::default()
    });
    assert!(feed(&mut parts, &shot).await);
    assert!(feed(&mut parts, &term).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["handle_screenshot_resp:s1|ok", "handle_terminal_response:true"]
    );
}

#[tokio::test]
async fn a_close_request_stops_a_running_voice_call() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let (tx, rx) = std::sync::mpsc::channel();
    parts.remote.stop_voice_call_sender = Some(tx);
    let mut msg = Message::new();
    msg.set_voice_call_request(VoiceCallRequest {
        is_connect: false,
        ..Default::default()
    });
    assert!(feed(&mut parts, &msg).await);
    assert!(rx.try_recv().is_ok(), "the audio capture thread is told to stop");
    assert!(parts.remote.stop_voice_call_sender.is_none());
    assert_eq!(parts.remote.handler.calls(), vec!["on_voice_call_closed:"]);
}

#[tokio::test]
async fn a_voice_call_response_is_matched_against_the_pending_request() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut stale = Message::new();
    stale.set_voice_call_response(VoiceCallResponse {
        accepted: true,
        req_timestamp: 5,
        ..Default::default()
    });
    assert!(feed(&mut parts, &stale).await);
    assert!(parts.remote.handler.calls().is_empty(), "no request pending");

    parts.remote.voice_call_request_timestamp = NonZeroI64::new(9);
    let mut wrong = Message::new();
    wrong.set_voice_call_response(VoiceCallResponse {
        accepted: true,
        req_timestamp: 5,
        ..Default::default()
    });
    assert!(feed(&mut parts, &wrong).await);
    assert!(parts.remote.handler.calls().is_empty(), "mismatched timestamp");
    assert!(parts.remote.voice_call_request_timestamp.is_none());

    parts.remote.voice_call_request_timestamp = NonZeroI64::new(9);
    let mut refused = Message::new();
    refused.set_voice_call_response(VoiceCallResponse {
        accepted: false,
        req_timestamp: 9,
        ..Default::default()
    });
    assert!(feed(&mut parts, &refused).await);
    assert_eq!(parts.remote.handler.calls(), vec!["on_voice_call_closed:"]);
}

#[tokio::test]
async fn unknown_and_unparsable_input_keeps_the_loop_running() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &Message::new()).await);
    assert!(
        parts
            .remote
            .handle_msg_from_peer(b"not a protobuf message", &mut parts.peer)
            .await
    );
    assert!(parts.remote.handler.calls().is_empty());
}
