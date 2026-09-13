use super::*;

#[test]
fn a_packet_declared_over_the_cap_ends_the_tunnel_before_it_arrives() {
    rt().block_on(async {
        use std::time::{Duration, Instant};
        let (ours, mut theirs) = local_pair().await;
        let addr = ours.peer_addr().unwrap();
        let t = Tunnel::new();
        t.claim();
        let _h = t.set_muxed_checking(Stream::Tcp(FramedStream::from(ours, addr)), NoUi::default(), ok_login());
        assert!(matches!(t.claim(), Claim::Muxed(_)));
        // The codec's three-byte header form, declaring one byte more
        // than the cap, and nothing behind it: an uncapped codec waits
        // for the whole packet and the tunnel stays up.
        let head = ((MAX_PACKET as u32 + 1) << 2) | 0x2;
        theirs.write_all(&head.to_le_bytes()[..3]).await.unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !matches!(t.claim(), Claim::Claimed) {
            assert!(Instant::now() < deadline, "tunnel still up: the oversized header was accepted");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
}

#[test]
fn claim_follows_the_tunnel_state() {
    let t = Tunnel::new();
    assert!(matches!(t.claim(), Claim::Claimed));
    t.set_failed();
    assert!(matches!(t.claim(), Claim::Claimed));
    t.set_legacy();
    assert!(matches!(t.claim(), Claim::Legacy));
}

#[test]
fn an_id_still_live_when_the_counter_comes_round_is_skipped() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        t.claim();
        let h = t.set_muxed_checking(ours, NoUi::default(), ok_login());
        let id_of = |ch: &PortForwardChannel| match &ch.union {
            Some(port_forward_channel::Union::Open(o)) => o.channel_id,
            other => panic!("expected open, got {:?}", other),
        };
        let (_a, sock_a) = local_pair().await;
        h.open("localhost", 80, sock_a, vec![]).unwrap();
        let a = id_of(&recv_frame(&mut peer).await);
        // 2^32 opens later the counter is back at A's id, and A is
        // still up. Handing that id out again would replace A's entry
        // here while the peer keeps routing it to A's socket.
        h.set_next_id(a);
        let (_b, sock_b) = local_pair().await;
        h.open("localhost", 80, sock_b, vec![]).unwrap();
        let b = id_of(&recv_frame(&mut peer).await);
        assert_ne!(b, a, "channel B was handed A's live id");
        assert_eq!(h.live_channels(), 2);
    });
}

#[test]
fn open_sends_open_then_pipelined_data_and_relays_replies() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        assert!(matches!(t.claim(), Claim::Claimed));
        let h = t.set_muxed_checking(ours, NoUi::default(), ok_login());
        let (mut app, sock) = local_pair().await;
        h.open("localhost", 80, sock, b"GET / HTTP/1.0\r\n\r\n".to_vec()).unwrap();
        let open = recv_frame(&mut peer).await;
        let id = match &open.union {
            Some(port_forward_channel::Union::Open(o)) => {
                assert_eq!((o.host.as_str(), o.port, o.window), ("localhost", 80, CHANNEL_WINDOW));
                o.channel_id
            }
            other => panic!("expected open, got {:?}", other),
        };
        let d = recv_frame(&mut peer).await;
        match &d.union {
            Some(port_forward_channel::Union::Data(d)) => assert_eq!(d.data, b"GET / HTTP/1.0\r\n\r\n".to_vec()),
            other => panic!("expected data, got {:?}", other),
        }
        peer.send(&opened_msg(id, true, "", CHANNEL_WINDOW)).await.unwrap();
        peer.send(&data_msg(id, Bytes::from_static(b"HTTP/1.0 200 OK\r\n"))).await.unwrap();
        let mut buf = [0u8; 17];
        app.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"HTTP/1.0 200 OK\r\n");
        peer.send(&close_msg(id)).await.unwrap();
        assert_eq!(app.read(&mut buf).await.unwrap(), 0);
    });
}

#[test]
fn dropping_the_tunnel_ends_the_peer_connection() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        assert!(matches!(t.claim(), Claim::Claimed));
        let h = t.set_muxed_checking(ours, NoUi::default(), ok_login());
        let (app, sock) = local_pair().await;
        h.open("localhost", 80, sock, Vec::new()).unwrap();
        let id = match recv_frame(&mut peer).await.union {
            Some(port_forward_channel::Union::Open(o)) => o.channel_id,
            other => panic!("expected open, got {:?}", other),
        };
        peer.send(&close_msg(id)).await.unwrap();
        drop(app);
        // The loop holds a handle of its own, so dropping ours proves
        // nothing; the listener's `Tunnel` is what must end the peer.
        drop(h);
        drop(t);
        let end = hbb_common::timeout(2000, peer.next()).await;
        assert!(
            matches!(end, Ok(None) | Ok(Some(Err(_)))),
            "peer still connected: {:?}",
            end
        );
    });
}
