use super::*;
use bytes::Bytes;
use hbb_common::{
    bytes_codec::BytesCodec,
    sodiumoxide::crypto::secretbox,
    tcp::{DynTcpStream, FramedStream},
    tokio,
    tokio_util::codec::Framed,
};
use std::{
    io::Result as IoResult,
    net::SocketAddr,
    pin::Pin,
    sync::{atomic::Ordering, Arc, Mutex},
    task::{Context, Poll, Waker},
};

mod blocking_io;
use blocking_io::BlockingIo;

const SECOND: std::time::Duration = std::time::Duration::from_secs(2);

fn addr() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

fn framed_over(io: BlockingIo) -> Stream {
    Stream::Tcp(FramedStream(
        Framed::new(DynTcpStream(Box::new(io)), BytesCodec::new()),
        addr(),
        None,
        0,
    ))
}

fn halves(stream: Stream) -> (ConnReader, ConnWriter) {
    match super::split(stream) {
        Ok(halves) => halves,
        Err(_) => panic!("a TCP stream must be splittable"),
    }
}

/// The defect this module exists for.
///
/// One owner cannot do both at once: while the connection loop awaits a send, it is not
/// awaiting the socket, so input and clipboard wait behind the picture. Here the write is
/// held open forever and the reader must still deliver, which is only possible because the
/// two halves do not share a lock.
#[tokio::test(flavor = "current_thread")]
async fn a_blocked_write_does_not_stop_the_reader() {
    let io = BlockingIo::new();
    io.block_writes();
    io.feed_frame(b"input");
    let writes_seen = io.writes_completed();

    let (mut reader, mut writer) = halves(framed_over(io));

    let write = tokio::spawn(async move {
        let _ = writer.send_bytes(Bytes::from(vec![0u8; 64])).await;
    });

    let got = tokio::time::timeout(SECOND, reader.next())
        .await
        .expect("the reader was starved by a blocked write")
        .expect("the stream ended")
        .expect("the frame did not decode");
    assert_eq!(&got[..], b"input");
    assert_eq!(
        writes_seen.load(Ordering::SeqCst),
        0,
        "the write completed, so this proves nothing about blocking"
    );

    write.abort();
}

/// Bytes that had already been pulled off the socket but not yet framed must survive the
/// split. They are whatever arrived in the same packet as the last message read before it.
#[tokio::test(flavor = "current_thread")]
async fn the_split_keeps_what_was_already_buffered() {
    let io = BlockingIo::new();
    io.feed_frame(b"first");
    io.feed_frame(b"second");

    let mut stream = framed_over(io);
    let first = tokio::time::timeout(SECOND, stream.next())
        .await
        .expect("no first message")
        .expect("the stream ended")
        .expect("the frame did not decode");
    assert_eq!(&first[..], b"first");

    // Both frames arrived together, so the second is sitting in the framed layer's read
    // buffer rather than in the socket.
    let (mut reader, _writer) = halves(stream);
    let second = tokio::time::timeout(SECOND, reader.next())
        .await
        .expect("the buffered message was lost by the split")
        .expect("the stream ended")
        .expect("the frame did not decode");
    assert_eq!(&second[..], b"second");
}

/// The dangerous one. Encryption uses a counter per direction, and the two halves each
/// carry their own copy of the pair. If a copy started from zero, or if the writer picked
/// up the reader's counter, the peer would fail to decrypt — which is what this asserts by
/// running a real peer on the other end of the connection.
#[tokio::test(flavor = "current_thread")]
async fn the_encryption_counters_do_not_cross_at_the_split() {
    let (a_io, b_io) = tokio::io::duplex(64 * 1024);
    let mut a = FramedStream::from(a_io, addr());
    let mut b = FramedStream::from(b_io, addr());
    let key = secretbox::gen_key();
    a.set_key(key.clone());
    b.set_key(key);

    // Run both counters up before splitting, so a half that reset to zero is caught.
    a.send_raw(b"a1".to_vec()).await.expect("a1");
    assert_eq!(&recv(&mut b).await[..], b"a1");
    b.send_raw(b"b1".to_vec()).await.expect("b1");
    assert_eq!(&recv(&mut a).await[..], b"b1");
    b.send_raw(b"b2".to_vec()).await.expect("b2");
    assert_eq!(&recv(&mut a).await[..], b"b2");

    let (mut reader, writer) = halves(Stream::Tcp(a));
    let ConnWriter::Tcp(mut writer) = writer;

    writer.send_raw(b"a2".to_vec()).await.expect("a2 after the split");
    assert_eq!(
        &recv(&mut b).await[..],
        b"a2",
        "the peer could not decrypt what the write half sent"
    );

    b.send_raw(b"b3".to_vec()).await.expect("b3");
    let got = tokio::time::timeout(SECOND, reader.next())
        .await
        .expect("no message after the split")
        .expect("the stream ended")
        .expect("the read half could not decrypt what the peer sent");
    assert_eq!(&got[..], b"b3");
}

async fn recv(s: &mut FramedStream) -> bytes::BytesMut {
    tokio::time::timeout(SECOND, s.next())
        .await
        .expect("nothing arrived")
        .expect("the stream ended")
        .expect("the frame did not decode")
}

/// A stream with bytes already half-written is handed back rather than split, and it comes
/// back as the stream the caller gave, not a husk.
#[tokio::test(flavor = "current_thread")]
async fn a_stream_with_a_pending_write_buffer_is_not_split() {
    let io = BlockingIo::new();
    io.block_writes();
    io.feed_frame(b"still readable");
    let mut stream = framed_over(io);

    // The socket refuses the bytes, so they stay in the framed layer's write buffer.
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        stream.send_bytes(Bytes::from(vec![7u8; 8])),
    )
    .await;

    let mut stream = match super::split(stream) {
        Err(s) => s,
        Ok(_) => panic!("a stream with a pending write buffer must be handed back"),
    };
    let got = tokio::time::timeout(SECOND, stream.next())
        .await
        .expect("the stream handed back is not usable")
        .expect("the stream ended")
        .expect("the frame did not decode");
    assert_eq!(&got[..], b"still readable");
}

/// The behaviour the split removes, kept as a description of it.
///
/// One owner cannot await a send and the socket at once — the borrow checker will not
/// even let it try — so a frame that was ready the whole time is only delivered after the
/// blocked send has been given up on. This passes before and after the change; it is here
/// to say what the coupling was, not to detect it.
#[tokio::test(flavor = "current_thread")]
async fn one_owner_cannot_read_while_its_write_is_blocked() {
    let io = BlockingIo::new();
    io.block_writes();
    io.feed_frame(b"input that is ready now");
    let mut stream = framed_over(io);

    let blocked = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        stream.send_bytes(Bytes::from(vec![0u8; 64])),
    )
    .await;
    assert!(blocked.is_err(), "the write was supposed to block");

    // Only now can the read be attempted at all.
    let got = tokio::time::timeout(SECOND, stream.next())
        .await
        .expect("nothing arrived")
        .expect("the stream ended")
        .expect("the frame did not decode");
    assert_eq!(&got[..], b"input that is ready now");
}
