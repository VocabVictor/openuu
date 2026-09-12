use super::*;

#[test]
fn frame_builders_set_the_expected_union_variant() {
    use base::message_proto::{message, port_forward_channel};
    let m = data_msg(7, Bytes::from_static(b"abc"));
    match m.union {
        Some(message::Union::PortForwardChannel(ch)) => match ch.union {
            Some(port_forward_channel::Union::Data(d)) => {
                assert_eq!(d.channel_id, 7);
                assert_eq!(d.data, b"abc".to_vec());
            }
            other => panic!("unexpected {:?}", other),
        },
        other => panic!("unexpected {:?}", other),
    }
    let m = opened_msg(3, false, "nope", CHANNEL_WINDOW);
    match m.union {
        Some(message::Union::PortForwardChannel(ch)) => match ch.union {
            Some(port_forward_channel::Union::Opened(o)) => {
                assert_eq!((o.channel_id, o.success, o.message.as_str(), o.window),
                           (3, false, "nope", CHANNEL_WINDOW));
            }
            other => panic!("unexpected {:?}", other),
        },
        other => panic!("unexpected {:?}", other),
    }
}

use base::message_proto::{message, port_forward_channel};
use hbb_common::tokio::{self, io::AsyncReadExt, io::AsyncWriteExt, sync::mpsc};
use std::sync::{Arc, Mutex};

struct Harness {
    data_rx: mpsc::Receiver<Message>,
    control_rx: mpsc::UnboundedReceiver<Message>,
    inbound_tx: mpsc::UnboundedSender<Inbound>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    local: tokio::io::DuplexStream,
    teardown: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

/// A channel whose "local socket" is one end of a duplex pipe and whose
/// "tunnel" is a pair of queues the test reads directly.
fn harness(id: i32, prebuf: Vec<u8>, initial_out: Vec<Bytes>) -> Harness {
    harness_on(id, prebuf, initial_out, watch::channel(false).0)
}

/// The channel subscribes to `teardown` here, so a test can hand in one
/// that has already been raised.
fn harness_on(id: i32, prebuf: Vec<u8>, initial_out: Vec<Bytes>, teardown: watch::Sender<bool>) -> Harness {
    let (data_tx, data_rx) = mpsc::channel(DATA_QUEUE_FRAMES);
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
    let (local, remote) = tokio::io::duplex(1 << 20);
    let (r, w) = tokio::io::split(remote);
    let credit = Arc::new(SendCredit::new(INITIAL_WINDOW));
    let window = Arc::new(Mutex::new(RecvWindow::new(CHANNEL_WINDOW)));
    let sink = FrameSink::Queued { data: data_tx, control: control_tx };
    let teardown_rx = teardown.subscribe();
    let task = tokio::spawn(run_channel(
        id, r, w, prebuf, initial_out, credit.clone(), window.clone(), inbound_rx, sink, teardown_rx,
    ));
    Harness { data_rx, control_rx, inbound_tx, credit, window, local, teardown, task }
}

// `PortForwardData.data` is generated as `bytes::Bytes` (hbb_common builds
// rust-protobuf with the bytes feature), so it converts with `to_vec()`, not
// `clone()`, and needs no wrapping when it becomes an `Inbound::Data`.
fn frame_kind(m: &Message) -> (&'static str, i32, Vec<u8>) {
    match &m.union {
        Some(message::Union::PortForwardChannel(ch)) => match &ch.union {
            Some(port_forward_channel::Union::Data(d)) => ("data", d.channel_id, d.data.to_vec()),
            Some(port_forward_channel::Union::Close(c)) => ("close", c.channel_id, vec![]),
            Some(port_forward_channel::Union::WindowUpdate(u)) => {
                ("window_update", u.channel_id, u.add.to_le_bytes().to_vec())
            }
            Some(port_forward_channel::Union::Open(o)) => ("open", o.channel_id, vec![]),
            Some(port_forward_channel::Union::Opened(o)) => ("opened", o.channel_id, vec![]),
            None => ("none", 0, vec![]),
            // `port_forward_channel::Union` is `#[non_exhaustive]` in the
            // generated protobuf code, so it needs a catch-all here even
            // though every current variant is already matched above.
            _ => ("other", 0, vec![]),
        },
        _ => ("other", 0, vec![]),
    }
}

#[test]
fn teardown_ends_a_channel_parked_on_a_socket_nobody_reads() {
    rt().block_on(async {
        // Nothing reads the local side, so once the duplex buffer is full
        // the socket relay parks in write_all; nothing writes it either,
        // so the tunnel relay parks in read. Neither is on the inbound
        // queue, which stays open here: teardown alone must end them.
        let mut h = harness(1, vec![], vec![]);
        for _ in 0..17 {
            h.inbound_tx.send(Inbound::Data(Bytes::from(vec![0u8; MAX_FRAME]))).unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        h.teardown.send_replace(true);
        let ended = tokio::time::timeout(std::time::Duration::from_millis(500), &mut h.task).await;
        assert!(ended.is_ok(), "channel task outlived the tunnel");
    });
}

#[test]
fn a_channel_subscribed_after_teardown_ends_at_once() {
    rt().block_on(async {
        // `open` can race `close_all`: this channel subscribes after the
        // signal went out, and its entry sits in a map that was already
        // cleared, so nothing will ever drop its inbound sender. No other
        // channel was live when the tunnel closed, either.
        let teardown = watch::channel(false).0;
        teardown.send_replace(true);
        let mut h = harness_on(1, vec![], vec![], teardown);
        let ended = tokio::time::timeout(std::time::Duration::from_millis(500), &mut h.task).await;
        assert!(ended.is_ok(), "late channel outlived the tunnel");
    });
}

#[test]
fn local_bytes_become_data_frames_capped_at_max_frame() {
    rt().block_on(async {
        let mut h = harness(1, vec![], vec![]);
        let payload = vec![7u8; MAX_FRAME + 10];
        h.local.write_all(&payload).await.unwrap();
        // INITIAL_WINDOW equals MAX_FRAME, so the first frame exhausts
        // it exactly; grant one minimum charge for the 10-byte tail.
        h.credit.add(MIN_FRAME_CHARGE);
        let mut got = Vec::new();
        while got.len() < payload.len() {
            let m = h.data_rx.recv().await.unwrap();
            let (kind, id, bytes) = frame_kind(&m);
            assert_eq!((kind, id), ("data", 1));
            assert!(bytes.len() <= MAX_FRAME);
            got.extend(bytes);
        }
        assert_eq!(got, payload);
    });
}

#[test]
fn prebuf_is_the_head_of_the_send_stream() {
    rt().block_on(async {
        let mut h = harness(2, b"head".to_vec(), vec![]);
        h.local.write_all(b"tail").await.unwrap();
        let mut got = Vec::new();
        while got.len() < 8 {
            let m = h.data_rx.recv().await.unwrap();
            got.extend(frame_kind(&m).2);
        }
        assert_eq!(got, b"headtail".to_vec());
    });
}

#[test]
fn send_side_stops_at_credit_and_resumes_on_add() {
    rt().block_on(async {
        let mut h = harness(3, vec![], vec![]);
        let payload = vec![1u8; INITIAL_WINDOW as usize + 5];
        h.local.write_all(&payload).await.unwrap();
        let mut got = 0usize;
        while got < INITIAL_WINDOW as usize {
            got += frame_kind(&h.data_rx.recv().await.unwrap()).2.len();
        }
        assert_eq!(got, INITIAL_WINDOW as usize);
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(50),
            h.data_rx.recv()
        )
        .await
        .is_err());
        // One minimum charge is enough to send the 5-byte tail.
        h.credit.add(MIN_FRAME_CHARGE);
        assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()).2.len(), 5);
    });
}

#[test]
fn inbound_data_is_written_and_window_update_follows_threshold() {
    rt().block_on(async {
        let mut h = harness(4, vec![], vec![Bytes::from_static(b"first")]);
        let mut buf = [0u8; 5];
        h.local.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"first");
        let chunk = Bytes::from(vec![9u8; UPDATE_THRESHOLD as usize]);
        assert!(h.window.lock().unwrap().accept(chunk.len()));
        h.inbound_tx.send(Inbound::Data(chunk.clone())).unwrap();
        let mut sink = vec![0u8; chunk.len()];
        h.local.read_exact(&mut sink).await.unwrap();
        let m = h.control_rx.recv().await.unwrap();
        let (kind, id, add) = frame_kind(&m);
        assert_eq!((kind, id), ("window_update", 4));
        let add = u32::from_le_bytes([add[0], add[1], add[2], add[3]]);
        // The 5-byte `initial` chunk drained a whole minimum charge.
        assert_eq!(add, UPDATE_THRESHOLD + MIN_FRAME_CHARGE);
    });
}

#[test]
fn local_eof_sends_close_exactly_once_after_the_data() {
    rt().block_on(async {
        let mut h = harness(5, vec![], vec![]);
        h.local.write_all(b"bye").await.unwrap();
        drop(h.local);
        assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()).0, "data");
        assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()), ("close", 5, vec![]));
        h.task.await.unwrap();
        assert!(h.data_rx.try_recv().is_err());
    });
}

#[test]
fn peer_close_ends_the_channel_without_echoing_close() {
    rt().block_on(async {
        let mut h = harness(6, vec![], vec![]);
        h.inbound_tx.send(Inbound::Close).unwrap();
        h.task.await.unwrap();
        assert!(h.data_rx.try_recv().is_err());
        assert!(h.control_rx.try_recv().is_err());
    });
}

#[test]
fn violation_signalled_by_the_demux_sends_close() {
    rt().block_on(async {
        let mut h = harness(7, vec![], vec![]);
        h.inbound_tx.send(Inbound::Violation).unwrap();
        assert_eq!(frame_kind(&h.data_rx.recv().await.unwrap()), ("close", 7, vec![]));
        h.task.await.unwrap();
    });
}

#[test]
fn dropped_tunnel_ends_the_channel_silently() {
    rt().block_on(async {
        let mut h = harness(8, vec![], vec![]);
        drop(h.inbound_tx);
        h.task.await.unwrap();
        assert!(h.data_rx.try_recv().is_err());
    });
}
