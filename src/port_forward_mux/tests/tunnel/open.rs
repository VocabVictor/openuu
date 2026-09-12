use super::*;

#[test]
fn every_open_precedes_its_own_channels_first_data() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        assert!(matches!(t.claim(), Claim::Claimed));
        let h = t.set_muxed(ours, NoUi::default());
        // Twenty channels, each with one byte of pipelined data behind
        // its open. An open on the control queue can lose the loop's
        // random tie-break to a data frame, so one channel would be a
        // coin flip; twenty make a wrong implementation fail every run.
        const N: usize = 20;
        let mut apps = Vec::new();
        for i in 0..N {
            let (app, sock) = local_pair().await;
            h.open("localhost", 1, sock, vec![i as u8]).unwrap();
            apps.push(app);
        }
        let mut opened = std::collections::HashSet::new();
        let mut seen_data = 0;
        while seen_data < N {
            let ch = recv_frame(&mut peer).await;
            match &ch.union {
                Some(port_forward_channel::Union::Open(o)) => {
                    assert!(opened.insert(o.channel_id), "duplicate open");
                }
                Some(port_forward_channel::Union::Data(d)) => {
                    assert!(
                        opened.contains(&d.channel_id),
                        "data for channel {} arrived before its open",
                        d.channel_id
                    );
                    seen_data += 1;
                }
                other => panic!("unexpected {:?}", other),
            }
        }
    });
}

#[test]
fn failed_open_closes_the_local_socket() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        t.claim();
        let h = t.set_muxed(ours, NoUi::default());
        let (mut app, sock) = local_pair().await;
        h.open("localhost", 1, sock, vec![]).unwrap();
        let id = match recv_frame(&mut peer).await.union {
            Some(port_forward_channel::Union::Open(o)) => o.channel_id,
            other => panic!("expected open, got {:?}", other),
        };
        peer.send(&opened_msg(id, false, "refused", 0)).await.unwrap();
        let mut buf = [0u8; 1];
        assert_eq!(app.read(&mut buf).await.unwrap(), 0);
    });
}

#[test]
fn a_refused_channel_is_reported_once_per_reason() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        assert!(matches!(t.claim(), Claim::Claimed));
        let ui = NoUi::default();
        let h = t.set_muxed(ours, ui.clone());
        for reason in ["unreachable", "unreachable", "no permission"] {
            let (mut app, sock) = local_pair().await;
            h.open("localhost", 1, sock, vec![]).unwrap();
            let id = match recv_frame(&mut peer).await.union {
                Some(port_forward_channel::Union::Open(o)) => o.channel_id,
                other => panic!("expected open, got {:?}", other),
            };
            peer.send(&opened_msg(id, false, reason, 0)).await.unwrap();
            let mut buf = [0u8; 1];
            assert_eq!(app.read(&mut buf).await.unwrap(), 0);
        }
        // A page load can have a dozen connections refused for one
        // reason; the user gets one dialog per reason, not per socket.
        assert_eq!(
            ui.messages(),
            vec!["unreachable".to_owned(), "no permission".to_owned()]
        );
    });
}

#[test]
fn a_refused_reason_is_reported_again_after_a_quiet_spell() {
    rt().block_on(async {
        let (ours, _peer) = stream_pair().await;
        let t = Tunnel::new();
        t.claim();
        let h = t.set_muxed(ours, NoUi::default());
        let t0 = Instant::now();
        let at = |secs: u64| t0 + std::time::Duration::from_secs(secs);
        let down = || "down".to_owned();
        assert_eq!(h.first_report_at(down(), at(0)), Some(down()));
        // A burst that keeps going keeps the dialog quiet ...
        assert_eq!(h.first_report_at(down(), at(8)), None);
        assert_eq!(h.first_report_at(down(), at(16)), None);
        // ... and one that stopped is reported afresh.
        assert_eq!(h.first_report_at(down(), at(27)), Some(down()));
        // The cap counts reasons still live, so it cannot silence the
        // window for good.
        for i in 0..MAX_REPORTED_OPEN_ERRORS {
            h.first_report_at(format!("reason {}", i), at(27));
        }
        assert_eq!(h.first_report_at("one more".to_owned(), at(27)), None);
        assert_eq!(
            h.first_report_at("one more".to_owned(), at(40)),
            Some("one more".to_owned())
        );
    });
}

#[test]
fn an_over_window_frame_drops_the_channel_at_once() {
    rt().block_on(async {
        let (ours, mut peer) = stream_pair().await;
        let t = Tunnel::new();
        t.claim();
        let h = t.set_muxed(ours, NoUi::default());
        let (_app, sock) = local_pair().await;
        h.open("localhost", 1, sock, vec![]).unwrap();
        let id = match recv_frame(&mut peer).await.union {
            Some(port_forward_channel::Union::Open(o)) => o.channel_id,
            other => panic!("expected open, got {:?}", other),
        };
        let mut ch = PortForwardChannel::new();
        ch.set_data(PortForwardData {
            channel_id: id,
            data: Bytes::from(vec![0u8; CHANNEL_WINDOW as usize + 1]),
            ..Default::default()
        });
        h.on_frame(ch.clone());
        // Gone on the spot: a peer that keeps sending past the window
        // can no longer queue anything for this channel.
        assert_eq!(h.live_channels(), 0);
        h.on_frame(ch);
        assert_eq!(h.live_channels(), 0);
    });
}

#[test]
fn tunnel_death_closes_channels_and_resets_state() {
    rt().block_on(async {
        let (ours, peer) = stream_pair().await;
        let t = Tunnel::new();
        t.claim();
        let h = t.set_muxed(ours, NoUi::default());
        let (mut app_a, sock_a) = local_pair().await;
        let (mut app_b, sock_b) = local_pair().await;
        h.open("localhost", 1, sock_a, vec![]).unwrap();
        h.open("localhost", 1, sock_b, vec![]).unwrap();
        drop(peer);
        // One tunnel is one failure domain: every channel on it ends.
        let mut buf = [0u8; 1];
        assert_eq!(app_a.read(&mut buf).await.unwrap(), 0);
        assert_eq!(app_b.read(&mut buf).await.unwrap(), 0);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(h.open("localhost", 1, local_pair().await.1, vec![]).is_err());
        // The next accept establishes again on the same `Tunnel`.
        assert!(matches!(t.claim(), Claim::Claimed));
        let (ours, mut peer) = stream_pair().await;
        let h = t.set_muxed(ours, NoUi::default());
        let (_app_c, sock_c) = local_pair().await;
        h.open("localhost", 1, sock_c, vec![]).unwrap();
        assert!(matches!(
            recv_frame(&mut peer).await.union,
            Some(port_forward_channel::Union::Open(_))
        ));
    });
}
