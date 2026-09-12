use super::connection::{Connection, Sender};
use crate::port_forward_mux::{
    charge, close_msg, effective_window, opened_msg, run_channel, FrameSink, Inbound, RecvWindow,
    SendCredit, CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS,
};
use hbb_common::{
    bytes::Bytes,
    log,
    timeout,
    tokio::{self, net::TcpStream, sync::{mpsc, watch}},
};
use base::message_proto::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const CONNECT_TIMEOUT_MS: u64 = 3000;

/// Before `opened` the controller may only have used `INITIAL_WINDOW`.
/// `charged` is the running total of `charge(len)`, not of raw lengths.
fn pending_fits(charged: usize, add_len: usize) -> bool {
    charged.saturating_add(charge(add_len) as usize) <= INITIAL_WINDOW as usize
}

struct Entry {
    inbound: mpsc::UnboundedSender<Inbound>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
}

/// The controlled side of one multiplexed tunnel. The main loop owns it and
/// forwards every `PortForwardChannel` frame here; each channel is a task.

mod mux;
mod channel;
use channel::*;
pub struct PortForwardMux {
    channels: HashMap<i32, Entry>,
    tx: Sender,
    login_target: String,
    /// Raised once, by `close_all`, for the channels its `clear` cannot reach:
    /// one parked on its target socket is not on the inbound queue.
    teardown: watch::Sender<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port_forward_mux::{CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS, MIN_FRAME_CHARGE};
    use hbb_common::{
        tokio::{
            self,
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
            sync::mpsc,
            time::Instant,
        },
    };
    use base::message_proto::{message, port_forward_channel};

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// An echo server standing in for the forward target.
    async fn echo_target() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = l.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    loop {
                        let n = s.read(&mut buf).await.unwrap_or(0);
                        if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        port
    }

    fn open(id: i32, port: u16) -> PortForwardChannel {
        let mut ch = PortForwardChannel::new();
        ch.set_open(PortForwardOpen {
            channel_id: id,
            host: "127.0.0.1".to_owned(),
            port: port as i32,
            window: CHANNEL_WINDOW,
            ..Default::default()
        });
        ch
    }

    fn data(id: i32, bytes: &[u8]) -> PortForwardChannel {
        let mut ch = PortForwardChannel::new();
        ch.set_data(PortForwardData {
            channel_id: id,
            data: Bytes::copy_from_slice(bytes),
            ..Default::default()
        });
        ch
    }

    fn close(id: i32) -> PortForwardChannel {
        let mut ch = PortForwardChannel::new();
        ch.set_close(PortForwardClose { channel_id: id, ..Default::default() });
        ch
    }

    async fn next_frame(rx: &mut mpsc::UnboundedReceiver<(Instant, Arc<Message>)>) -> PortForwardChannel {
        let (_, m) = rx.recv().await.unwrap();
        match &m.union {
            Some(message::Union::PortForwardChannel(ch)) => ch.clone(),
            other => panic!("unexpected {:?}", other),
        }
    }

    fn opened(ch: &PortForwardChannel) -> (i32, bool) {
        match &ch.union {
            Some(port_forward_channel::Union::Opened(o)) => (o.channel_id, o.success),
            other => panic!("expected opened, got {:?}", other),
        }
    }

    fn data_of(ch: &PortForwardChannel) -> (i32, Vec<u8>) {
        match &ch.union {
            Some(port_forward_channel::Union::Data(d)) => (d.channel_id, d.data.to_vec()),
            other => panic!("expected data, got {:?}", other),
        }
    }

    #[test]
    fn open_connects_and_echoes_pipelined_data() {
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
    fn unreachable_target_fails_open_and_discards_pipelined_data() {
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
    fn permission_denied_refuses_without_spawning() {
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
    fn a_revoked_permission_refuses_new_channels_and_keeps_live_ones() {
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
    fn close_while_connecting_sends_no_opened() {
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
    fn over_window_data_closes_only_that_channel() {
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
    fn an_over_window_frame_drops_the_channel_at_once() {
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
    fn open_to_a_target_other_than_the_login_target_is_refused() {
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
    fn demux_admits_only_the_initial_window_before_opened() {
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
    fn pending_bytes_are_bounded_by_initial_window_before_opened() {
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

    /// A target that accepts and hangs up at once, so every channel ends on
    /// the target's EOF — the case where only the next `open` frees the entry.
    async fn drop_target() -> u16 {
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
    fn open_frees_dead_entries_so_the_cap_counts_live_channels() {
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
}
