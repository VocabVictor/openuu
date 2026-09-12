use super::*;

#[test]
pub(super) fn open_connects_and_echoes_pipelined_data() {
    rt().block_on(async {
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        mux.handle(data(1, b"ping"), || true);
        assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
        assert_eq!(data_of(&next_frame(&mut rx).await), (1, b"ping".to_vec()));
        mux.handle(close(1), || true);
    });
}

#[test]
pub(super) fn unreachable_target_fails_open_and_discards_pipelined_data() {
    rt().block_on(async {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        mux.handle(data(1, b"lost"), || true);
        assert_eq!(opened(&next_frame(&mut rx).await), (1, false));
        assert!(tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await.is_err());
    });
}

#[test]
pub(super) fn permission_denied_refuses_without_spawning() {
    rt().block_on(async {
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || false);
        assert_eq!(opened(&next_frame(&mut rx).await), (1, false));
        assert_eq!(mux.live_channels(), 0);
    });
}

#[test]
pub(super) fn a_revoked_permission_refuses_new_channels_and_keeps_live_ones() {
    rt().block_on(async {
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
        // `enable-tunnel` is consulted per `open`, so turning it off
        // mid-session stops new channels; the live one keeps relaying.
        mux.handle(open(2, port), || false);
        assert_eq!(opened(&next_frame(&mut rx).await), (2, false));
        mux.handle(data(1, b"still relayed"), || false);
        assert_eq!(data_of(&next_frame(&mut rx).await), (1, b"still relayed".to_vec()));
        assert_eq!(mux.live_channels(), 1);
    });
}

#[test]
pub(super) fn close_while_connecting_sends_no_opened() {
    rt().block_on(async {
        // `close` is queued before the task is first polled. Its `select!` is
        // biased towards the command arm, so even a connect that completes on
        // that same poll loses: no `opened` may ever be sent.
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        mux.handle(close(1), || true);
        assert!(tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await.is_err());
        assert_eq!(mux.live_channels(), 0);
    });
}

#[test]
pub(super) fn over_window_data_closes_only_that_channel() {
    rt().block_on(async {
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        mux.handle(open(2, port), || true);
        let mut seen = 0;
        while seen < 2 {
            opened(&next_frame(&mut rx).await);
            seen += 1;
        }
        let too_much = vec![0u8; CHANNEL_WINDOW as usize + 1];
        mux.handle(data(1, &too_much), || true);
        let ch = next_frame(&mut rx).await;
        match &ch.union {
            Some(port_forward_channel::Union::Close(c)) => assert_eq!(c.channel_id, 1),
            other => panic!("expected close, got {:?}", other),
        }
        mux.handle(data(2, b"still fine"), || true);
        assert_eq!(data_of(&next_frame(&mut rx).await), (2, b"still fine".to_vec()));
    });
}

#[test]
pub(super) fn an_over_window_frame_drops_the_channel_at_once() {
    rt().block_on(async {
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        opened(&next_frame(&mut rx).await);
        let too_much = vec![0u8; CHANNEL_WINDOW as usize + 1];
        mux.handle(data(1, &too_much), || true);
        // Gone before the channel task has run: whatever the peer keeps
        // sending for this id can no longer queue anything.
        assert_eq!(mux.live_channels(), 0);
        mux.handle(data(1, &too_much), || true);
        assert_eq!(mux.live_channels(), 0);
        let ch = next_frame(&mut rx).await;
        match &ch.union {
            Some(port_forward_channel::Union::Close(c)) => assert_eq!(c.channel_id, 1),
            other => panic!("expected close, got {:?}", other),
        }
    });
}

#[test]
pub(super) fn open_to_a_target_other_than_the_login_target_is_refused() {
    rt().block_on(async {
        let a = echo_target().await;
        let b = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", a));
        mux.handle(open(1, a), || true);
        assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
        // Approval was for target a; b needs a login of its own.
        mux.handle(open(2, b), || true);
        let ch = next_frame(&mut rx).await;
        match &ch.union {
            Some(port_forward_channel::Union::Opened(o)) => {
                assert_eq!((o.channel_id, o.success), (2, false));
                assert!(!o.message.is_empty());
            }
            other => panic!("expected opened, got {:?}", other),
        }
        assert_eq!(mux.live_channels(), 1);
    });
}

#[test]
pub(super) fn demux_admits_only_the_initial_window_before_opened() {
    rt().block_on(async {
        let port = echo_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        mux.handle(open(1, port), || true);
        // The channel task has not run yet: the demultiplexer alone
        // decides what may sit in the queue before `opened`.
        assert_eq!(mux.recv_window_remaining(1), Some(INITIAL_WINDOW));
        assert_eq!(opened(&next_frame(&mut rx).await), (1, true));
        assert_eq!(mux.recv_window_remaining(1), Some(CHANNEL_WINDOW));
    });
}

#[test]
pub(super) fn pending_bytes_are_bounded_by_initial_window_before_opened() {
    // A loopback connect completes before a task can observe "connecting",
    // so the bound is pinned on the pure predicate the task uses.
    assert!(pending_fits(0, INITIAL_WINDOW as usize));
    assert!(pending_fits(
        INITIAL_WINDOW as usize - MIN_FRAME_CHARGE as usize,
        1
    ));
    // A 1-byte frame costs a whole minimum charge here too.
    assert!(!pending_fits(
        INITIAL_WINDOW as usize - MIN_FRAME_CHARGE as usize + 1,
        1
    ));
    assert!(!pending_fits(usize::MAX, 1));
}
