use super::*;
use super::super::test_support::{next_message, try_next_message};
use hbb_common::tokio::net::TcpListener;

async fn loopback_stream() -> super::super::super::Stream {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let _client = TcpStream::connect(addr).await.unwrap();
    let (server_side, peer_addr) = listener.accept().await.unwrap();
    super::super::super::Stream::from(server_side, peer_addr)
}

fn permission_of(msg: &Message) -> (Permission, bool) {
    match &msg.union {
        Some(message::Union::Misc(m)) => match &m.union {
            Some(misc::Union::PermissionInfo(p)) => {
                (p.permission.enum_value().unwrap(), p.enabled)
            }
            other => panic!("expected PermissionInfo, got {other:?}"),
        },
        other => panic!("expected Misc, got {other:?}"),
    }
}

#[tokio::test]
async fn build_wires_the_message_channel_to_the_connection_inner() {
    let stream = loopback_stream().await;
    let (mut conn, mut ch) = Connection::build(stream, 9601, std::sync::Weak::new(), None, None);
    assert_eq!(conn.inner.id(), 9601);
    assert!(!conn.authorized);
    assert!(!conn.closed);
    let mut msg = Message::new();
    msg.set_test_delay(TestDelay::default());
    conn.inner.send(Arc::new(msg));
    let (_, queued) = ch.rx.try_recv().expect("queued on rx");
    assert!(matches!(queued.union, Some(message::Union::TestDelay(_))));
    assert!(ch.rx_video.try_recv().is_err());
    assert!(ch.rx_from_cm.try_recv().is_err());
}

#[tokio::test]
async fn build_takes_the_permission_flags_from_the_options() {
    let stream = loopback_stream().await;
    let (conn, _ch) = Connection::build(stream, 9602, std::sync::Weak::new(), None, None);
    let none = None;
    assert_eq!(
        conn.keyboard,
        Connection::permission(keys::OPTION_ENABLE_KEYBOARD, &none)
    );
    assert_eq!(
        conn.file,
        Connection::permission(keys::OPTION_ENABLE_FILE_TRANSFER, &none)
    );
    assert_eq!(
        conn.privacy_mode,
        Connection::permission(keys::OPTION_ENABLE_PRIVACY_MODE, &none)
    );
}

#[tokio::test]
async fn only_denied_permissions_are_announced() {
    let (mut conn, mut controller) = Connection::for_test(9603).await;
    conn.keyboard = true;
    conn.clipboard = true;
    conn.audio = false;
    conn.file = true;
    conn.restart = false;
    conn.recording = true;
    conn.block_input = true;
    conn.privacy_mode = true;
    conn.send_denied_permissions().await;
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::Audio, false)
    );
    assert_eq!(
        permission_of(&next_message(&mut controller).await),
        (Permission::Restart, false)
    );
    assert!(try_next_message(&mut controller, 100).await.is_none());
}

#[tokio::test]
async fn finish_tells_the_cm_and_closes_the_peer_stream() {
    let mut parts = Connection::for_test_parts(9604).await;
    let (_tx, mut rx_from_cm) = mpsc::unbounded_channel::<ipc::Data>();
    parts.conn.finish(&mut rx_from_cm).await;
    assert!(matches!(parts.rx_to_cm.try_recv(), Ok(ipc::Data::Close)));
    // The connection is consumed, so the peer sees its stream close.
    let closed = timeout(3_000, parts.controller.next())
        .await
        .expect("stream closed within 3 s");
    assert!(closed.is_none());
}
