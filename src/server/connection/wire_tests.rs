//! What the two forms of a connection promise a file transfer.

use super::*;
use base::{
    fs::MsgSink,
    message_proto::{FileResponse, FileTransferBlock},
};
use hbb_common::{tcp::FramedStream, Stream};

fn addr() -> std::net::SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// A connection over a socket that holds a single byte, so everything after the first
/// write is still queued when the assertions run.
fn blocked_wire() -> (wire::Wire, FramedStream) {
    let (ours, theirs) = hbb_common::tokio::io::duplex(1);
    (
        wire::Wire::Whole(Stream::Tcp(FramedStream::from(ours, addr()))),
        FramedStream::from(theirs, addr()),
    )
}

/// A file block carrying its sequence number, the way a transfer numbers them.
fn block(n: i32) -> Message {
    let mut response = FileResponse::new();
    response.set_block(FileTransferBlock {
        id: 1,
        file_num: n,
        data: vec![0u8; 8].into(),
        ..Default::default()
    });
    let mut msg = Message::new();
    msg.set_file_response(response);
    msg
}

fn block_number(bytes: &[u8]) -> i32 {
    let msg = Message::parse_from_bytes(bytes).expect("not a Message");
    match msg.union {
        Some(message::Union::FileResponse(r)) => match r.union {
            Some(base::message_proto::file_response::Union::Block(b)) => b.file_num,
            other => panic!("expected a block, got {other:?}"),
        },
        other => panic!("expected a FileResponse, got {other:?}"),
    }
}

/// The one that matters: out of order is a corrupt file, not a slow one.
///
/// A hundred blocks are sent through the sink while the socket takes almost nothing, so
/// nearly all of them are queued rather than written, and they must still arrive in the
/// order the transfer produced them.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn blocks_keep_their_order_when_the_writing_half_is_a_task() {
    let (mut w, mut peer) = blocked_wire();
    assert!(w.split_for_video(), "a TCP connection must split");

    for n in 0..100 {
        w.send_msg(block(n)).await.expect("queueing a block");
    }

    for n in 0..100 {
        let bytes = peer
            .next_timeout(2000)
            .await
            .unwrap_or_else(|| panic!("block {n} never arrived"))
            .expect("the frame did not decode");
        assert_eq!(block_number(&bytes), n, "blocks arrived out of order");
    }
}

/// The same promise on the form that never splits, so the two are known to agree.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn blocks_keep_their_order_when_the_connection_owns_its_socket() {
    let (mut w, mut peer) = blocked_wire();
    let sender = hbb_common::tokio::spawn(async move {
        for n in 0..100 {
            w.send_msg(block(n)).await.expect("sending a block");
        }
        w
    });

    for n in 0..100 {
        let bytes = peer
            .next_timeout(2000)
            .await
            .unwrap_or_else(|| panic!("block {n} never arrived"))
            .expect("the frame did not decode");
        assert_eq!(block_number(&bytes), n, "blocks arrived out of order");
    }
    let _w = sender.await.expect("the sender finished");
}

/// Splitting is for sessions with video in them; the others keep the whole socket, which
/// is what lets port forwarding put it in raw mode after the loop has ended.
#[test]
fn the_whole_form_is_what_a_connection_starts_as() {
    let (w, _peer) = blocked_wire();
    assert!(matches!(w, wire::Wire::Whole(_)));
    assert!(w.writer().is_none(), "nothing is queueing until it splits");
}
