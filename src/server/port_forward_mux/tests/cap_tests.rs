use super::*;

/// A target that accepts and hangs up at once, so every channel ends on
/// the target's EOF — the case where only the next `open` frees the entry.
pub(super) async fn drop_target() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (s, _) = l.accept().await.unwrap();
            drop(s);
        }
    });
    port
}

#[test]
pub(super) fn open_frees_dead_entries_so_the_cap_counts_live_channels() {
    rt().block_on(async {
        let port = drop_target().await;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut mux = PortForwardMux::new(tx, format!("127.0.0.1:{}", port));
        for id in 1..=(MAX_CHANNELS as i32 * 2) {
            mux.handle(open(id, port), || true);
            assert_eq!(opened(&next_frame(&mut rx).await), (id, true));
            // The task sends `close` on the target's EOF and exits; the
            // entry is dead until the next `open` drops it.
            let ch = next_frame(&mut rx).await;
            match &ch.union {
                Some(port_forward_channel::Union::Close(c)) => assert_eq!(c.channel_id, id),
                other => panic!("expected close, got {:?}", other),
            }
            tokio::task::yield_now().await;
        }
    });
}
