use super::*;
use super::super::test_support::next_message;

fn misc(union: misc::Union) -> Message {
    let mut m = Misc::new();
    m.union = Some(union);
    let mut msg = Message::new();
    msg.set_misc(m);
    msg
}

#[tokio::test]
async fn chat_message_is_forwarded_to_the_connection_manager() {
    let mut parts = Connection::for_test_parts(9301).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.chat_unanswered = false;

    let msg = misc(misc::Union::ChatMessage(ChatMessage {
        text: "hello".to_owned(),
        ..Default::default()
    }));
    assert!(parts.conn.on_message(msg).await);

    match parts.rx_to_cm.try_recv().expect("forwarded to cm") {
        ipc::Data::ChatMessage { text } => assert_eq!(text, "hello"),
        _ => panic!("expected ipc::Data::ChatMessage"),
    }
    assert!(parts.conn.chat_unanswered);
}

#[tokio::test]
async fn quick_launch_is_denied_without_keyboard_permission() {
    let (mut conn, mut controller) = Connection::for_test(9302).await;
    conn.authorize_for_test(AuthConnType::Remote);
    conn.keyboard = false;
    let request = r#"{"request_id":"r-1","command":"calc"}"#.to_owned();

    assert!(conn.on_message(misc(misc::Union::QuickLaunchRequest(request.clone()))).await);

    let reply = next_message(&mut controller).await;
    match &reply.union {
        Some(message::Union::Misc(m)) => match &m.union {
            Some(misc::Union::QuickLaunchResponse(res)) => {
                assert_eq!(res, &crate::quick_launch::denied(&request));
                assert!(res.contains("Quick launch requires an authorized control session"));
            }
            other => panic!("expected QuickLaunchResponse, got {:?}", other),
        },
        other => panic!("expected Misc, got {:?}", other),
    }
}
