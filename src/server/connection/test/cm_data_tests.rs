use super::*;
use super::super::test_support::{next_message, try_next_message};

fn misc_of(msg: &Message) -> &misc::Union {
    match &msg.union {
        Some(message::Union::Misc(m)) => m.union.as_ref().expect("misc union"),
        other => panic!("expected Misc, got {other:?}"),
    }
}

fn permission_of(msg: &Message) -> (Permission, bool) {
    match misc_of(msg) {
        misc::Union::PermissionInfo(p) => (p.permission.enum_value().unwrap(), p.enabled),
        other => panic!("expected PermissionInfo, got {other:?}"),
    }
}

#[tokio::test]
async fn a_chat_message_from_the_cm_reaches_the_peer_and_counts_as_answered() {
    let (mut conn, mut controller) = Connection::for_test(9401).await;
    conn.chat_unanswered = true;
    assert!(
        conn.handle_cm_data(ipc::Data::ChatMessage {
            text: "hi".to_owned()
        })
        .await
    );
    match misc_of(&next_message(&mut controller).await) {
        misc::Union::ChatMessage(c) => assert_eq!(c.text, "hi"),
        other => panic!("expected ChatMessage, got {other:?}"),
    }
    assert!(!conn.chat_unanswered);
}

#[tokio::test]
async fn close_from_the_cm_tells_the_peer_not_to_retry_and_ends_the_loop() {
    let mut parts = Connection::for_test_parts(9402).await;
    parts.conn.chat_unanswered = true;
    assert!(!parts.conn.handle_cm_data(ipc::Data::Close).await);
    match misc_of(&next_message(&mut parts.controller).await) {
        misc::Union::CloseReason(r) => assert_eq!(r, "Closed manually by the peer"),
        other => panic!("expected CloseReason, got {other:?}"),
    }
    assert!(parts.conn.closed);
    assert!(!parts.conn.chat_unanswered);
    assert!(matches!(parts.rx_to_cm.try_recv(), Ok(ipc::Data::Close)));
}

#[tokio::test]
async fn an_expected_cm_error_is_ignored_and_any_other_ends_the_loop() {
    let (mut conn, _controller) = Connection::for_test(9403).await;
    assert!(
        conn.handle_cm_data(ipc::Data::CmErr("expected".to_owned()))
            .await
    );
    assert!(!conn.closed);
    assert!(!conn.handle_cm_data(ipc::Data::CmErr("gone".to_owned())).await);
    assert!(conn.closed);
}

#[tokio::test]
async fn raw_bytes_from_the_cm_are_written_to_the_stream_as_is() {
    let (mut conn, mut controller) = Connection::for_test(9404).await;
    let mut misc = Misc::new();
    misc.set_close_reason("raw".to_owned());
    let mut msg = Message::new();
    msg.set_misc(misc);
    let bytes = msg.write_to_bytes().unwrap();
    assert!(conn.handle_cm_data(ipc::Data::RawMessage(bytes)).await);
    match misc_of(&next_message(&mut controller).await) {
        misc::Union::CloseReason(r) => assert_eq!(r, "raw"),
        other => panic!("expected CloseReason, got {other:?}"),
    }
}

#[tokio::test]
async fn a_privacy_mode_state_becomes_a_back_notification() {
    let (mut conn, mut controller) = Connection::for_test(9405).await;
    let data = ipc::Data::PrivacyModeState((
        9405,
        privacy_mode::PrivacyModeState::OffByPeer,
        "impl-x".to_owned(),
    ));
    assert!(conn.handle_cm_data(data).await);
    match misc_of(&next_message(&mut controller).await) {
        misc::Union::BackNotification(n) => {
            assert_eq!(
                n.privacy_mode_state(),
                back_notification::PrivacyModeState::PrvOffByPeer
            );
            assert_eq!(n.impl_key, "impl-x");
        }
        other => panic!("expected BackNotification, got {other:?}"),
    }
}

#[tokio::test]
async fn closing_the_voice_call_from_the_cm_notifies_the_peer_and_the_cm() {
    let mut parts = Connection::for_test_parts(9406).await;
    parts.conn.voice_calling = true;
    assert!(
        parts
            .conn
            .handle_cm_data(ipc::Data::CloseVoiceCall(String::new()))
            .await
    );
    assert!(!parts.conn.voice_calling);
    assert!(matches!(
        parts.rx_to_cm.try_recv(),
        Ok(ipc::Data::CloseVoiceCall(_))
    ));
    match &next_message(&mut parts.controller).await.union {
        Some(message::Union::VoiceCallRequest(r)) => assert!(!r.is_connect),
        other => panic!("expected VoiceCallRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn file_results_for_another_connection_are_ignored() {
    let (mut conn, mut controller) = Connection::for_test(9407).await;
    let data = ipc::Data::FileReadDone {
        id: 1,
        file_num: 0,
        conn_id: 9999,
    };
    assert!(conn.handle_cm_data(data).await);
    assert!(try_next_message(&mut controller, 100).await.is_none());
}

#[tokio::test]
async fn a_simple_permission_toggle_updates_the_flag_and_tells_the_peer() {
    let (mut conn, mut controller) = Connection::for_test(9408).await;
    conn.restart = true;
    conn.handle_switch_permission("restart".to_owned(), false).await;
    assert!(!conn.restart);
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::Restart, false)
    );
    conn.handle_switch_permission("recording".to_owned(), true).await;
    assert!(conn.recording);
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::Recording, true)
    );
}

#[tokio::test]
async fn keyboard_and_clipboard_toggles_work_without_a_server() {
    let (mut conn, mut controller) = Connection::for_test(9409).await;
    conn.handle_switch_permission("keyboard".to_owned(), false).await;
    assert!(!conn.keyboard);
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::Keyboard, false)
    );
    conn.handle_switch_permission("clipboard".to_owned(), false).await;
    assert!(!conn.clipboard);
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::Clipboard, false)
    );
}

#[tokio::test]
async fn an_unknown_permission_name_changes_nothing() {
    let (mut conn, mut controller) = Connection::for_test(9410).await;
    let before = (conn.keyboard, conn.clipboard, conn.audio, conn.file);
    conn.handle_switch_permission("telepathy".to_owned(), false).await;
    assert_eq!(before, (conn.keyboard, conn.clipboard, conn.audio, conn.file));
    assert!(try_next_message(&mut controller, 100).await.is_none());
}

#[tokio::test]
async fn granting_privacy_mode_updates_the_flag_and_tells_the_peer() {
    let (mut conn, mut controller) = Connection::for_test(9411).await;
    conn.privacy_mode = false;
    conn.handle_switch_permission("privacy_mode".to_owned(), true).await;
    assert!(conn.privacy_mode);
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::PrivacyMode, true)
    );
}
