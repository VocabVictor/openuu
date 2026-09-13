use super::*;

fn voice_call_request(is_connect: bool, req_timestamp: i64) -> Message {
    let mut msg = Message::new();
    msg.set_voice_call_request(VoiceCallRequest {
        is_connect,
        req_timestamp,
        ..Default::default()
    });
    msg
}

#[tokio::test]
async fn voice_call_request_notifies_the_connection_manager() {
    let mut parts = Connection::for_test_parts(9401).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);

    assert!(parts.conn.on_message(voice_call_request(true, 1234)).await);

    assert!(matches!(
        parts.rx_to_cm.try_recv().expect("cm notified"),
        ipc::Data::VoiceCallIncoming
    ));
    assert_eq!(
        parts.conn.voice_call_request_timestamp.map(|t| t.get()),
        Some(1234)
    );
}

#[tokio::test]
async fn voice_call_hangup_closes_the_call() {
    let mut parts = Connection::for_test_parts(9402).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.voice_calling = true;

    assert!(parts.conn.on_message(voice_call_request(false, 0)).await);

    assert!(matches!(
        parts.rx_to_cm.try_recv().expect("cm notified"),
        ipc::Data::CloseVoiceCall(_)
    ));
    assert!(!parts.conn.voice_calling);
}
