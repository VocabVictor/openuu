use super::{mpsc, socket_client, tokio, IceRoute, ICE_DEDUP_WINDOW, MAX_PENDING_REMOTE_ICE};
use super::punch::answers_webrtc_only;
use hbb_common::tcp::new_listener;
use std::net::SocketAddr;

// A SOCKS proxy makes `connect_tcp_local` dial the proxy and ignore the local address, so
// nothing these two assert can hold. Read once, from the same global config production reads.
fn proxied() -> bool {
    hbb_common::config::Config::get_socks().is_some()
}

/// Both held while their addresses are read, so the pair cannot be the same port - which
/// `SO_REUSEPORT` would let bind twice rather than refuse, leaving the tests degenerate.
async fn free_loopback_pair() -> (SocketAddr, SocketAddr) {
    let (a, b) = (
        tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
        tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
    );
    (a.local_addr().unwrap(), b.local_addr().unwrap())
}

fn queue(route: &mut IceRoute, candidate: &str) -> bool {
    route.queue(candidate.to_owned())
}

#[test]
fn the_re_sent_copy_does_not_spend_a_queue_slot() {
    // Two slots, three sends: without the dedup the re-send takes the second and "relay",
    // the one that traverses NAT, is the one refused.
    let (tx, mut rx) = mpsc::channel::<String>(2);
    let mut route = IceRoute::new(tx);
    for _ in 0..2 {
        assert!(queue(&mut route, "host"));
    }
    assert!(queue(&mut route, "relay"));
    let mut queued = Vec::new();
    while let Ok(candidate) = rx.try_recv() {
        queued.push(candidate);
    }
    assert_eq!(queued, vec!["host".to_owned(), "relay".to_owned()]);
}

#[test]
fn a_candidate_the_full_queue_refused_is_not_remembered() {
    let (tx, mut rx) = mpsc::channel::<String>(1);
    let mut route = IceRoute::new(tx);
    assert!(queue(&mut route, "host"));
    assert!(!queue(&mut route, "relay"));
    // The re-send is the only repair for a refused candidate; remembering it would swallow it.
    assert_eq!(rx.try_recv().ok(), Some("host".to_owned()));
    assert!(queue(&mut route, "relay"));
    assert_eq!(rx.try_recv().ok(), Some("relay".to_owned()));
}

#[test]
fn a_re_send_is_skipped_while_the_original_is_still_queued() {
    let (tx, mut rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
    let mut route = IceRoute::new(tx);
    for i in 0..MAX_PENDING_REMOTE_ICE {
        assert!(queue(&mut route, &format!("candidate-{}", i)));
    }
    assert!(queue(&mut route, "candidate-0"));
    let mut queued = 0;
    while rx.try_recv().is_ok() {
        queued += 1;
    }
    assert_eq!(queued, MAX_PENDING_REMOTE_ICE);
}

#[test]
fn the_window_forgets_in_arrival_order() {
    let (tx, mut rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
    let mut route = IceRoute::new(tx);
    for i in 0..=ICE_DEDUP_WINDOW {
        assert!(queue(&mut route, &format!("candidate-{}", i)));
        assert!(rx.try_recv().is_ok());
    }
    // The oldest digest made room for the newest, so its re-send is admitted again.
    assert!(queue(&mut route, "candidate-0"));
    assert!(rx.try_recv().is_ok());
    // A recent one is still skipped.
    let recent = format!("candidate-{}", ICE_DEDUP_WINDOW);
    assert!(queue(&mut route, &recent));
    assert!(rx.try_recv().is_err());
}

// The second way in that the repeat punch opens: a punch reaching a peer already in SYN_SENT
// is answered by that socket rather than reset, and the two ends come up on one connection.
// A punch that misses the crossing is reset outright here, loopback having no NAT to absorb
// it and no round trip to hide behind - so a single punch lands only by luck, and repeating
// is what makes it land at all. That is the premise of the repeat, asserted directly. A round
// that misses costs one loopback RST, so rounds are cheap and there are many.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_punch_that_meets_the_peers_syn_connects_both_ends() {
    // The crossing needs both connects genuinely in flight at once. Loopback answers a SYN to
    // a port nobody is listening on with an instant RST, so on one CPU the first connect runs
    // to completion before the second is scheduled and no round can ever cross - a property of
    // the box, which this test cannot tell apart from a broken punch.
    if proxied() || std::thread::available_parallelism().map_or(true, |cpus| cpus.get() < 2) {
        return;
    }
    for _ in 0..256 {
        let (a, b) = free_loopback_pair().await;
        // Held for the whole crossing, because production always has one here and the design
        // rests on which of the two the kernel hands the connection to: the punch and the
        // peer's SYN share a four-tuple exactly, the listener only matches the address, and
        // the punch has to win that or every crossing would be swallowed as a plain accept.
        let listener = new_listener(a, true).await.unwrap();
        let to_b = tokio::spawn(socket_client::connect_tcp_local(b, Some(a), 3000));
        let to_a = tokio::spawn(socket_client::connect_tcp_local(a, Some(b), 3000));
        let (at_a, at_b) = tokio::join!(to_b, to_a);
        let (Ok(Ok(mut at_a)), Ok(Ok(mut at_b))) = (at_a, at_b) else {
            continue;
        };
        at_a.send_bytes(bytes::Bytes::from_static(b"punch"))
            .await
            .unwrap();
        let got = at_b.next_timeout(3000).await.unwrap().unwrap();
        assert_eq!(&got[..], b"punch", "both ends must share one connection");
        assert!(
            hbb_common::timeout(200, listener.accept()).await.is_err(),
            "the crossing must reach the punch, not be accepted as an inbound connection"
        );
        return;
    }
    panic!("no punch met the peer's SYN in 256 rounds on a machine that can cross them");
}

// The punch binds the address the listener already holds, so it has to go through the same
// `connect_tcp_local` production uses - a punch built by hand here would still pass if
// `new_socket` ever stopped setting the reuse flags, while every real punch failed to bind.
// The peer's view of the source port is what proves the bind took: a fallback to an ephemeral
// one would connect just as happily.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_punch_binds_the_address_the_listener_holds() {
    if proxied() {
        return;
    }
    // `free_loopback_pair` hands back ports it no longer holds, so another process can take
    // one in between; retry rather than fail for something the punch had no part in.
    for _ in 0..8 {
        let (local, peer_addr) = free_loopback_pair().await;
        let (Ok(listener), Ok(peer)) = (
            new_listener(local, true).await,
            new_listener(peer_addr, true).await,
        ) else {
            continue;
        };
        let punch = tokio::spawn(socket_client::connect_tcp_local(
            peer_addr,
            Some(local),
            1500,
        ));
        let (_peer_side, seen_as) = hbb_common::timeout(3000, peer.accept())
            .await
            .expect("the punch must reach the peer")
            .unwrap();
        assert_eq!(
            seen_as.port(),
            local.port(),
            "the punch must leave from the address the listener holds, not an ephemeral one"
        );
        // Held, not asserted and dropped: the coexistence below is only exercised while this
        // socket is still on the address, which is the state production spends its window in.
        let _punched = punch.await.unwrap().expect("the punch must connect");

        let dialed = tokio::spawn(tokio::net::TcpStream::connect(local));
        let accepted = hbb_common::timeout(3000, listener.accept()).await;
        assert!(
            matches!(accepted, Ok(Ok(_))),
            "the listener must still take connections while a punch shares its address: {accepted:?}"
        );
        assert!(dialed.await.unwrap().is_ok());
        return;
    }
    panic!("could not hold two free loopback addresses in 8 tries");
}

// The schedule on its own, against a paused clock: the window is CONNECT_TIMEOUT long, and
// what these pin is where inside it the punches fall, which no socket could show.
#[tokio::test(start_paused = true)]
async fn the_punches_end_on_one_at_the_deadline() {
    use super::{punch_until, PUNCH_GRACE, PUNCH_INTERVAL, PUNCH_MAX_INTERVAL};
    use hbb_common::{anyhow::anyhow, config::CONNECT_TIMEOUT};
    use std::time::Duration;
    use tokio::time::Instant;

    let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
    let start = Instant::now();
    let until = start + Duration::from_millis(CONNECT_TIMEOUT);
    let mut punches = Vec::new();
    // A gateway that answers with RST: every punch fails the moment it is made.
    let met = punch_until::<(), _, _>(until, peer, |ms| {
        punches.push((Instant::now(), ms));
        async { Err(anyhow!("RST")) }
    })
    .await;
    assert!(met.is_none());
    assert_eq!(
        Instant::now(),
        until,
        "must return the moment the window closes, not a backoff later"
    );
    // Tokio rounds every sleep up to the next millisecond.
    let slack = Duration::from_millis(1);
    assert!(punches[0].0 - start <= Duration::from_secs_f32(PUNCH_INTERVAL) + slack);
    for pair in punches.windows(2) {
        assert!(
            pair[1].0 - pair[0].0 <= Duration::from_secs_f32(PUNCH_MAX_INTERVAL) + slack,
            "no gap in the window may exceed the backoff ceiling: {pair:?}"
        );
    }
    assert_eq!(
        *punches.last().unwrap(),
        (until, PUNCH_GRACE),
        "the window must end on a punch, given the whole grace"
    );
}

#[tokio::test(start_paused = true)]
async fn a_punch_in_flight_runs_the_grace_past_the_deadline_and_no_further() {
    use super::{punch_until, PUNCH_GRACE};
    use hbb_common::{anyhow::anyhow, config::CONNECT_TIMEOUT};
    use std::time::Duration;
    use tokio::time::Instant;

    let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
    let until = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT);
    let mut punches = 0;
    // A gateway that drops the SYN in silence: the punch sits in SYN_SENT for all it is given.
    let met = punch_until::<(), _, _>(until, peer, |ms| {
        punches += 1;
        async move {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            Err(anyhow!("timed out"))
        }
    })
    .await;
    assert!(met.is_none());
    assert_eq!(
        punches, 1,
        "a punch held in SYN_SENT is the only one the window needs"
    );
    assert_eq!(
        Instant::now(),
        until + Duration::from_millis(PUNCH_GRACE),
        "must return when the grace runs out, not a backoff later"
    );
}

// A controller that offered WebRTC still dials the punched address when this side could not
// answer; that dial needs the TCP punch and its listener, so only a real answer skips them.
#[test]
fn an_unanswered_webrtc_offer_still_gets_the_tcp_punch() {
    assert!(!answers_webrtc_only(""));
    assert!(answers_webrtc_only("v=0"));
}
