use super::*;

#[test]
fn many_channels_echo_concurrently() {
    rt().block_on(async {
        let (_t, h, port) = muxed_tunnel().await;
        let mut apps = Vec::new();
        for i in 0..20u8 {
            let (app, sock) = local_pair().await;
            h.open("127.0.0.1", port as i32, sock, vec![i]).unwrap();
            apps.push(app);
        }
        // Twenty channels round-trip concurrently: channel 0 streams 4 MiB
        // while the other nineteen each exchange one byte.
        // Read and write the bulk socket from separate tasks: the echo can only
        // drain if this side keeps reading while it writes.
        let bulk = vec![0xAB; 4 << 20];
        let (mut bulk_rd, mut bulk_wr) = apps.remove(0).into_split();
        let bulk_reader = {
            let bulk = bulk.clone();
            tokio::spawn(async move {
                let mut back = vec![0u8; bulk.len() + 1];
                bulk_rd.read_exact(&mut back).await.unwrap();
                assert_eq!(back[0], 0);
                assert_eq!(&back[1..], &bulk[..]);
            })
        };
        let bulk_writer = {
            let bulk = bulk.clone();
            tokio::spawn(async move {
                bulk_wr.write_all(&bulk).await.unwrap();
                // Hold the write half open: dropping it half-closes the
                // socket, which ends the whole channel by design.
                bulk_wr
            })
        };
        for (i, app) in apps.iter_mut().enumerate() {
            let mut b = [0u8; 1];
            tokio::time::timeout(std::time::Duration::from_secs(2), app.read_exact(&mut b))
                .await
                .expect("small channel starved")
                .unwrap();
            assert_eq!(b[0], (i + 1) as u8);
        }
        bulk_reader.await.unwrap();
        let _bulk_wr = bulk_writer.await.unwrap();
    });
}

#[test]
fn a_channel_opened_during_a_bulk_transfer_is_served_promptly() {
    rt().block_on(async {
        let (_t, h, port) = muxed_tunnel().await;
        let (bulk_app, bulk_sock) = local_pair().await;
        h.open("127.0.0.1", port as i32, bulk_sock, vec![]).unwrap();
        let bulk = vec![0xAB; 4 << 20];
        let (mut bulk_rd, mut bulk_wr) = bulk_app.into_split();
        let bulk_writer = {
            let bulk = bulk.clone();
            tokio::spawn(async move {
                bulk_wr.write_all(&bulk).await.unwrap();
                // Holding the write half open: dropping it half-closes
                // the socket, which ends the channel by design.
                bulk_wr
            })
        };
        // Wait until a mebibyte is back, so the bulk channel is
        // demonstrably mid-flight before anything else is opened.
        let mut back = vec![0u8; 1 << 20];
        bulk_rd.read_exact(&mut back).await.unwrap();
        let (mut app, sock) = local_pair().await;
        h.open("127.0.0.1", port as i32, sock, vec![42]).unwrap();
        let mut b = [0u8; 1];
        tokio::time::timeout(std::time::Duration::from_secs(2), app.read_exact(&mut b))
            .await
            .expect("a channel opened during a bulk transfer starved")
            .unwrap();
        assert_eq!(b[0], 42);
        let mut rest = vec![0u8; bulk.len() - (1 << 20)];
        bulk_rd.read_exact(&mut rest).await.unwrap();
        let _bulk_wr = bulk_writer.await.unwrap();
    });
}

#[test]
fn a_local_half_close_ends_the_whole_channel() {
    rt().block_on(async {
        let (_t, h, port) = muxed_tunnel().await;
        let (app, sock) = local_pair().await;
        h.open("127.0.0.1", port as i32, sock, vec![]).unwrap();
        let (mut rd, wr) = app.into_split();
        // Dropping the write half is a shutdown(SHUT_WR). Supporting it
        // needs a direction flag on the close frame; today's raw pipe
        // drops both directions on either EOF too, and this matches it.
        drop(wr);
        let mut buf = [0u8; 1];
        assert_eq!(rd.read(&mut buf).await.unwrap(), 0);
    });
}

#[test]
fn tail_before_close_is_delivered_in_both_directions() {
    rt().block_on(async {
        let (_t, h, port) = muxed_tunnel().await;
        let (app, sock) = local_pair().await;
        h.open("127.0.0.1", port as i32, sock, vec![]).unwrap();
        let payload = vec![7u8; 300 * 1024];
        let (mut rd, mut wr) = app.into_split();
        let reader = {
            let payload = payload.clone();
            tokio::spawn(async move {
                let mut back = vec![0u8; payload.len()];
                rd.read_exact(&mut back).await.unwrap();
                assert_eq!(back, payload);
                rd
            })
        };
        wr.write_all(&payload).await.unwrap();
        // The full echo proves every byte reached the target ahead of anything
        // else; only then close, and the peer's `close` must follow cleanly.
        let mut rd = reader.await.unwrap();
        drop(wr);
        let mut one = [0u8; 1];
        assert_eq!(rd.read(&mut one).await.unwrap(), 0);
    });
}

#[test]
fn one_byte_frames_never_trip_the_window() {
    rt().block_on(async {
        let (_t, h, port) = muxed_tunnel().await;
        let (mut app, sock) = local_pair().await;
        h.open("127.0.0.1", port as i32, sock, vec![]).unwrap();
        for i in 0..5000u32 {
            app.write_all(&[(i % 251) as u8]).await.unwrap();
            app.flush().await.unwrap();
        }
        let mut back = vec![0u8; 5000];
        app.read_exact(&mut back).await.unwrap();
        for (i, b) in back.iter().enumerate() {
            assert_eq!(*b, (i as u32 % 251) as u8);
        }
    });
}

#[test]
fn sequential_connections_far_beyond_max_channels_all_succeed() {
    rt().block_on(async {
        let (_t, h, port) = muxed_tunnel().await;
        for i in 0..(MAX_CHANNELS * 3) {
            let (mut app, sock) = local_pair().await;
            h.open("127.0.0.1", port as i32, sock, vec![i as u8]).unwrap();
            let mut b = [0u8; 1];
            app.read_exact(&mut b)
                .await
                .unwrap_or_else(|e| panic!("round {i} of {}: {e}", MAX_CHANNELS * 3));
            assert_eq!(b[0], i as u8, "round {i}");
            drop(app);
            // The controller's entry goes when its coordinator task exits,
            // which takes a cancel and a join; wait for it rather than
            // trusting a single yield, or the cap trips around round 256.
            while h.live_channels() != 0 {
                tokio::task::yield_now().await;
            }
        }
    });
}
