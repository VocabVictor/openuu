use super::*;
use super::super::test_support::{next_message, try_next_message};

fn misc_of(msg: &Message) -> &misc::Union {
    match &msg.union {
        Some(message::Union::Misc(m)) => m.union.as_ref().expect("misc union"),
        other => panic!("expected Misc, got {other:?}"),
    }
}

fn queued(msg: Message) -> Arc<Message> {
    Arc::new(msg)
}

fn long_ago() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(120))
        .expect("instant in the past")
}

#[tokio::test]
async fn a_queued_message_is_written_to_the_peer() {
    let (mut conn, mut controller) = Connection::for_test(9501).await;
    let mut msg = Message::new();
    msg.set_test_delay(TestDelay {
        last_delay: 7,
        ..Default::default()
    });
    assert!(conn.send_queued(Instant::now(), queued(msg)).await);
    match &next_message(&mut controller).await.union {
        Some(message::Union::TestDelay(t)) => assert_eq!(t.last_delay, 7),
        other => panic!("expected TestDelay, got {other:?}"),
    }
}

#[tokio::test]
async fn a_stale_audio_frame_is_dropped() {
    let (mut conn, mut controller) = Connection::for_test(9502).await;
    let mut msg = Message::new();
    msg.set_audio_frame(AudioFrame::default());
    assert!(conn.send_queued(long_ago(), queued(msg)).await);
    assert!(try_next_message(&mut controller, 100).await.is_none());
}

#[tokio::test]
async fn a_queued_stop_service_closes_the_connection() {
    let (mut conn, mut controller) = Connection::for_test(9503).await;
    let mut misc = Misc::new();
    misc.set_stop_service(true);
    let mut msg = Message::new();
    msg.set_misc(misc);
    assert!(!conn.send_queued(Instant::now(), queued(msg)).await);
    assert!(conn.closed);
    match misc_of(&next_message(&mut controller).await) {
        misc::Union::CloseReason(r) => assert_eq!(r, "Closed manually by the peer"),
        other => panic!("expected CloseReason, got {other:?}"),
    }
}

#[tokio::test]
async fn the_test_delay_tick_probes_once_until_the_peer_answers() {
    let (mut conn, mut controller) = Connection::for_test(9504).await;
    conn.network_delay = 42;
    assert!(conn.on_test_delay_tick(Instant::now()).await);
    assert!(conn.last_test_delay.is_some());
    match &next_message(&mut controller).await.union {
        Some(message::Union::TestDelay(t)) => assert_eq!(t.last_delay, 42),
        other => panic!("expected TestDelay, got {other:?}"),
    }
    assert!(conn.on_test_delay_tick(Instant::now()).await);
    assert!(try_next_message(&mut controller, 100).await.is_none());
}

#[tokio::test]
async fn the_test_delay_tick_times_out_a_silent_peer() {
    let (mut conn, _controller) = Connection::for_test(9505).await;
    assert!(!conn.on_test_delay_tick(long_ago()).await);
    assert!(conn.closed);
}

#[tokio::test]
async fn the_second_tick_keeps_an_active_connection() {
    let (mut conn, mut controller) = Connection::for_test(9506).await;
    assert!(conn.on_second_tick(0).await);
    assert!(!conn.closed);
    assert!(try_next_message(&mut controller, 100).await.is_none());
}

#[tokio::test]
async fn the_second_tick_disconnects_after_the_inactivity_limit() {
    let (mut conn, mut controller) = Connection::for_test(9507).await;
    conn.auto_disconnect_timer = Some((long_ago(), 1));
    assert!(!conn.on_second_tick(0).await);
    assert!(conn.closed);
    match misc_of(&next_message(&mut controller).await) {
        misc::Union::CloseReason(r) => assert_eq!(r, "Connection failed due to inactivity"),
        other => panic!("expected CloseReason, got {other:?}"),
    }
}
