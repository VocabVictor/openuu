use super::*;
use std::time::Duration;

async fn connected_pair() -> (Arc<UdpSocket>, Arc<UdpSocket>) {
    let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    a.connect(b.local_addr().unwrap()).await.unwrap();
    b.connect(a.local_addr().unwrap()).await.unwrap();
    (Arc::new(a), Arc::new(b))
}

async fn establish() -> ((KcpStream, Stream), (KcpStream, Stream)) {
    let (a, b) = connected_pair().await;
    let (accept_res, connect_res) = tokio::join!(
        KcpStream::accept(b, Duration::from_secs(5), None),
        KcpStream::connect(a, Duration::from_secs(5))
    );
    (
        connect_res.expect("connect over loopback"),
        accept_res.expect("accept over loopback"),
    )
}

// The full client path over real loopback sockets: handshake through the kcp_io
// pumps, framed data both ways, then a graceful close. The endpoint guard stays
// alive across the stream drop so the FIN can go out, and the peer's framed
// stream must end (BrokenPipe from the kcp reader) instead of hanging.
#[tokio::test]
async fn test_kcp_stream_loopback_roundtrip_and_close() {
    let ((_guard_a, mut stream_a), (_guard_b, mut stream_b)) = establish().await;

    stream_a
        .send_bytes(Bytes::from_static(b"ping"))
        .await
        .unwrap();
    let got = stream_b.next_timeout(5000).await.unwrap().unwrap();
    assert_eq!(&got[..], b"ping");

    stream_b
        .send_bytes(Bytes::from_static(b"pong"))
        .await
        .unwrap();
    let got = stream_a.next_timeout(5000).await.unwrap().unwrap();
    assert_eq!(&got[..], b"pong");

    drop(stream_a);
    match stream_b.next_timeout(10_000).await {
        None | Some(Err(_)) => {}
        Some(Ok(data)) => panic!("unexpected data after close: {:?}", data),
    }
}

// A writer that queues many frames and closes immediately must not cost the
// reader any of them: every frame arrives intact, in order, before end-of-stream.
// This is the client-side pin for the kcp-sys close-tail-drain semantics, through
// the real BytesCodec framing rustdesk sessions use.
#[tokio::test]
async fn test_kcp_stream_close_delivers_all_frames() {
    let ((_guard_a, mut tx), (_guard_b, mut rx)) = establish().await;

    const N: usize = 50;
    let payload = vec![7u8; 32 * 1024];
    for _ in 0..N {
        tx.send_bytes(Bytes::from(payload.clone())).await.unwrap();
    }
    drop(tx);

    let mut got = 0usize;
    loop {
        match rx.next_timeout(10_000).await {
            Some(Ok(data)) => {
                assert_eq!(data.len(), payload.len(), "frame boundary broken");
                assert!(data.iter().all(|&b| b == 7), "frame content corrupted");
                got += 1;
            }
            // BrokenPipe (kcp reader end) or timeout-None both end the stream.
            None | Some(Err(_)) => break,
        }
    }
    assert_eq!(got, N, "graceful close lost frames");
}

// Socket errors on the connected UDP socket (ICMP unreachable after the peer
// vanishes) are advisory: the io loop must treat them as loss - keep accepting
// writes, keep running - rather than tearing the session down. Whether the OS
// actually surfaces ECONNREFUSED here is platform-dependent; either way the
// session must stay alive for this window.
#[tokio::test]
async fn test_kcp_io_treats_socket_errors_as_loss() {
    let ((_guard_a, mut stream_a), (guard_b, stream_b)) = establish().await;

    // Kill the peer entirely: endpoint stops, socket closes.
    drop(stream_b);
    drop(guard_b);
    tokio::time::sleep(Duration::from_millis(50)).await;

    for _ in 0..10 {
        stream_a
            .send_bytes(Bytes::from_static(b"into the void"))
            .await
            .expect("socket errors must be treated as loss, not stream failure");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

// The connect deadline must hold when nothing answers: no hang, prompt error.
#[tokio::test]
async fn test_kcp_connect_timeout_without_peer() {
    let (a, _b) = connected_pair().await;
    let start = tokio::time::Instant::now();
    let res = KcpStream::connect(a, Duration::from_millis(600)).await;
    assert!(res.is_err(), "connect must fail with no peer endpoint");
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "connect did not honor its deadline"
    );
}
