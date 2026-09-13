use super::*;

fn misc(union: misc::Union) -> Message {
    let mut msg = Message::new();
    msg.set_misc(Misc {
        union: Some(union),
        ..Default::default()
    });
    msg
}

fn permission(permission: Permission, enabled: bool) -> Message {
    misc(misc::Union::PermissionInfo(PermissionInfo {
        permission: permission.into(),
        enabled,
        ..Default::default()
    }))
}

#[tokio::test]
async fn audio_format_and_chat_go_to_the_audio_thread_and_the_ui() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &misc(misc::Union::AudioFormat(AudioFormat::default()))).await);
    assert!(matches!(
        parts.rx_media.try_recv(),
        Ok(MediaData::AudioFormat(_))
    ));
    let chat = misc(misc::Union::ChatMessage(ChatMessage {
        text: "hi".to_owned(),
        ..Default::default()
    }));
    assert!(feed(&mut parts, &chat).await);
    assert_eq!(parts.remote.handler.calls(), vec!["new_message:hi"]);
}

#[tokio::test]
async fn permission_info_updates_the_session_flags_and_the_ui() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &permission(Permission::Keyboard, false)).await);
    assert!(feed(&mut parts, &permission(Permission::Clipboard, false)).await);
    assert!(feed(&mut parts, &permission(Permission::Audio, true)).await);
    assert!(feed(&mut parts, &permission(Permission::Restart, true)).await);
    assert!(feed(&mut parts, &permission(Permission::BlockInput, false)).await);
    assert!(feed(&mut parts, &permission(Permission::PrivacyMode, true)).await);
    assert!(!*parts.remote.handler.server_keyboard_enabled.read().unwrap());
    assert!(!*parts.remote.handler.server_clipboard_enabled.read().unwrap());
    assert_eq!(
        parts.remote.handler.calls(),
        vec![
            "set_permission:keyboard=false",
            "set_permission:clipboard=false",
            "set_permission:audio=true",
            "set_permission:restart=true",
            "set_permission:block_input=false",
            "set_permission:privacy_mode=true",
        ]
    );
}

#[tokio::test]
async fn a_file_permission_change_is_reported_unless_this_is_a_file_transfer_session() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &permission(Permission::File, false)).await);
    assert!(!*parts.remote.handler.server_file_transfer_enabled.read().unwrap());
    assert!(parts.remote.handler.has_call("set_permission:file=false"));

    parts.remote.handler.lc.write().unwrap().conn_type = ConnType::FILE_TRANSFER;
    parts.remote.handler.calls_clear();
    assert!(feed(&mut parts, &permission(Permission::File, false)).await);
    assert!(
        parts.remote.handler.calls().is_empty(),
        "a file transfer session losing the file permission stops here"
    );
}

#[tokio::test]
async fn recording_permission_is_stored_and_reported() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &permission(Permission::Recording, true)).await);
    assert!(parts.remote.handler.lc.read().unwrap().record_permission);
    assert!(parts.remote.handler.has_call("set_permission:recording=true"));
}

#[tokio::test]
async fn switch_display_sets_the_display_geometry() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let msg = misc(misc::Union::SwitchDisplay(SwitchDisplay {
        display: 1,
        x: 10,
        y: 20,
        width: 800,
        height: 600,
        cursor_embedded: true,
        ..Default::default()
    }));
    assert!(feed(&mut parts, &msg).await);
    assert!(parts.remote.handler.has_call("set_display:10,20,800,600,true,1"));
}

#[tokio::test]
async fn a_close_reason_shows_the_error_and_ends_the_loop() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let msg = misc(misc::Union::CloseReason("bye".to_owned()));
    assert!(!feed(&mut parts, &msg).await);
    assert!(parts.remote.sent_close_reason);
    // the retry flag comes from check_if_retry, which is not under test here
    assert!(parts.remote.handler.has_call("msgbox:error|Connection Error|bye|"));
}

#[tokio::test]
async fn elevation_messages_drive_the_uac_message_boxes() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &misc(misc::Union::Uac(true))).await);
    assert!(feed(&mut parts, &misc(misc::Union::Uac(false))).await);
    assert!(feed(&mut parts, &misc(misc::Union::ElevationResponse("".to_owned()))).await);
    assert!(feed(&mut parts, &misc(misc::Union::ElevationResponse("denied".to_owned()))).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec![
            "msgbox:on-uac|Prompt|Please wait for confirmation of UAC...||false",
            "cancel_msgbox:on-uac",
            "cancel_msgbox:wait-uac",
            "cancel_msgbox:elevation-error",
            "msgbox:wait-uac||||false",
            "cancel_msgbox:wait-uac",
            "msgbox:elevation-error|Elevation Error|denied||false",
        ]
    );
}

#[tokio::test]
async fn portable_service_running_reports_success_only_when_requested() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &misc(misc::Union::PortableServiceRunning(true))).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["portable_service_running:true"]
    );
    parts.remote.elevation_requested = true;
    parts.remote.handler.calls_clear();
    assert!(feed(&mut parts, &misc(misc::Union::PortableServiceRunning(true))).await);
    assert!(parts.remote.handler.has_call("msgbox:custom-nocancel-success"));
}

#[tokio::test]
async fn supported_encoding_quick_launch_and_follow_display_are_applied() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let enc = misc(misc::Union::SupportedEncoding(SupportedEncoding {
        h264: true,
        ..Default::default()
    }));
    assert!(feed(&mut parts, &enc).await);
    assert!(parts.remote.handler.lc.read().unwrap().supported_encoding.h264);

    let small = misc(misc::Union::QuickLaunchResponse("{}".to_owned()));
    let huge = misc(misc::Union::QuickLaunchResponse("x".repeat(2 * 1024 * 1024 + 1)));
    assert!(feed(&mut parts, &small).await);
    assert!(feed(&mut parts, &huge).await);
    assert!(feed(&mut parts, &misc(misc::Union::FollowCurrentDisplay(2))).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["quick_launch_response:2", "set_current_display:2"],
        "responses above 2 MiB are dropped"
    );
}
