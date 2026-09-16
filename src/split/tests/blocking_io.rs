//! A socket whose writes can be held open indefinitely while its reads keep delivering.
//!
//! Real blocking needs a full kernel send buffer, which takes megabytes and a peer that
//! refuses to read, and is timing-dependent either way. This holds the write at
//! `Poll::Pending` on purpose, which is the same thing the loop sees and is deterministic.

use super::*;
use bytes::BytesMut;
use hbb_common::tokio_util::codec::Encoder;
use hbb_common::tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct State {
    to_deliver: BytesMut,
    writes_blocked: bool,
    read_waker: Option<Waker>,
}

#[derive(Clone)]
pub struct BlockingIo {
    state: Arc<Mutex<State>>,
    writes_completed: Arc<AtomicUsize>,
}

impl BlockingIo {
    pub fn new() -> Self {
        Self {
            state: Default::default(),
            writes_completed: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Every write from now on stays pending, as a write to a link that has stopped
    /// draining does.
    pub fn block_writes(&self) {
        self.state.lock().unwrap().writes_blocked = true;
    }

    /// Queue one frame for the reader, encoded the way the codec would put it on the wire.
    pub fn feed_frame(&self, payload: &[u8]) {
        let mut buf = BytesMut::new();
        BytesCodec::new()
            .encode(Bytes::copy_from_slice(payload), &mut buf)
            .expect("encoding a test frame");
        let mut state = self.state.lock().unwrap();
        state.to_deliver.extend_from_slice(&buf);
        if let Some(waker) = state.read_waker.take() {
            waker.wake();
        }
    }

    /// How many writes have actually reached the socket. A blocking test that lets a write
    /// through is not testing blocking.
    pub fn writes_completed(&self) -> Arc<AtomicUsize> {
        self.writes_completed.clone()
    }
}

impl AsyncRead for BlockingIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let mut state = self.state.lock().unwrap();
        if state.to_deliver.is_empty() {
            state.read_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let n = buf.remaining().min(state.to_deliver.len());
        let chunk = state.to_deliver.split_to(n);
        buf.put_slice(&chunk);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for BlockingIo {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        if self.state.lock().unwrap().writes_blocked {
            // No waker is registered on purpose: nothing will ever wake this, which is the
            // worst case the connection loop has to survive.
            return Poll::Pending;
        }
        self.writes_completed.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        if self.state.lock().unwrap().writes_blocked {
            return Poll::Pending;
        }
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}
