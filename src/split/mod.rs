//! Splitting a connection into a reading half and a writing half.
//!
//! # Why this lives here and not in `hbb_common`
//!
//! The connection's main loop owns one `Stream` and both reads and writes through it, so
//! a write that blocks holds the loop and input, clipboard and probe replies wait behind
//! the picture. Fixing that needs two independent halves. The obvious place for the split
//! is `hbb_common`, which owns `Stream` — but that is an upstream submodule we cannot push
//! to, and `docs/submodule-decision.md` records the decision not to fork it.
//!
//! # What this module stands on, and what happens when it breaks
//!
//! It is built entirely out of items upstream happens to expose today:
//!
//! * `tcp::FramedStream`'s four fields are `pub` (the framed socket, the peer address, the
//!   optional `Encrypt`, the send timeout in milliseconds).
//! * `tcp::Encrypt`'s three fields are `pub` and it derives `Clone`. It is a key, a
//!   **send** counter and a **receive** counter, and the two counters are independent.
//! * `tcp::DynTcpStream`'s field is `pub`, and its inner trait object is `AsyncRead +
//!   AsyncWrite + Send + Sync`, which is what `tokio::io::split` needs.
//! * `Stream::close_webrtc` does nothing on a TCP stream, which is what lets the emptied
//!   husk of one be dropped after its socket has been taken out.
//!
//! None of that is a promise upstream has made. When a pointer move changes any of it,
//! **this file stops compiling**, which is the failure we want: it cannot silently corrupt
//! a session. The repair is either to follow the new shape here, or — if the shape has
//! become impossible — to delete this module and let `Connection` go back to owning one
//! `Stream`, which still works and is what the WebSocket transport does today. Section 5
//! of `docs/submodule-decision.md` says to reconsider forking if that repair is needed
//! more than about twice.
//!
//! # What is not split
//!
//! **WebSocket**, because `WsFramedStream`'s fields are private and it exposes no
//! accessor, so there is no way to reach its socket from here. It keeps the single-owner
//! path, which is acceptable because nothing uses the WebSocket transport by default.
//!
//! **WebRTC**, not yet: its handle clones freely, so the split is nearly free there, but
//! `Stream` has a `Drop` that closes the session, and two `Stream`s over one connection
//! would close it when the first half is dropped. That needs its own commit and its own
//! test.

use hbb_common::{protobuf, ResultType, Stream};

mod null_io;
mod tcp_split;

#[cfg(test)]
mod tests;

pub use tcp_split::{TcpReader, TcpWriter};

/// The reading half of a split connection.
pub enum ConnReader {
    Tcp(TcpReader),
}

/// The writing half of a split connection.
pub enum ConnWriter {
    Tcp(TcpWriter),
}

/// Split a connection in two, or hand it back unchanged.
///
/// `Err(stream)` is not a failure: it is a transport this module cannot split, and the
/// caller keeps the single-owner loop it has always used. Returning the stream rather than
/// consuming it is what makes the unsupported path cost nothing.
pub fn split(mut stream: Stream) -> Result<(ConnReader, ConnWriter), Stream> {
    // `Stream` implements `Drop`, so its variants cannot be destructured. Take the socket
    // out through the `&mut` instead and leave an inert one in its place; dropping the
    // husk is safe because `close_webrtc` does nothing on a TCP stream.
    let taken = match &mut stream {
        Stream::Tcp(framed) => std::mem::replace(framed, null_io::null_framed_stream()),
        _ => return Err(stream),
    };
    match tcp_split::split(taken) {
        Ok((r, w)) => Ok((ConnReader::Tcp(r), ConnWriter::Tcp(w))),
        Err(framed) => {
            // Put it back, so the caller gets the stream it gave us rather than a husk.
            if let Stream::Tcp(slot) = &mut stream {
                let _ = std::mem::replace(slot, framed);
            }
            Err(stream)
        }
    }
}

impl ConnReader {
    pub async fn next(&mut self) -> Option<Result<bytes::BytesMut, std::io::Error>> {
        match self {
            ConnReader::Tcp(r) => r.next().await,
        }
    }
}

impl ConnWriter {
    pub async fn send(&mut self, msg: &impl protobuf::Message) -> ResultType<()> {
        match self {
            ConnWriter::Tcp(w) => w.send(msg).await,
        }
    }

    /// Bytes that are already an encoded message: they still get the length header and
    /// the encryption `send` would have applied.
    pub async fn send_raw(&mut self, bytes: Vec<u8>) -> ResultType<()> {
        match self {
            ConnWriter::Tcp(w) => w.send_raw(bytes).await,
        }
    }

    pub fn set_send_timeout(&mut self, ms: u64) {
        match self {
            ConnWriter::Tcp(w) => w.set_send_timeout(ms),
        }
    }
}
