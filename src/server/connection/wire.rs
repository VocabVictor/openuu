//! What a connection talks through.
//!
//! A remote-desktop session hands its socket to a writer task so that a blocked write
//! cannot hold the loop; every other kind of session keeps the socket whole, because the
//! coupling only hurts where video is. Both live behind this type.
//!
//! It exists as an enum rather than a `Stream` plus an optional writer so that **the
//! compiler finds every place that sends**. The alternative — leaving an emptied husk in
//! place after handing the socket away — would let a missed call site write into nothing
//! and lose messages with no error at all, which is the exact failure this whole change
//! is meant to remove rather than introduce.
//!
//! This commit adds only the whole form; splitting comes next.

// The split form is constructed in the next commit, which is also where this goes.
#![allow(dead_code)]

use super::*;
use crate::stream_split::ConnReader;
use base::message_proto::Message;
use hbb_common::{ResultType, Stream};

pub(super) enum Wire {
    /// One owner, reads and writes. What every session starts as.
    Whole(Stream),
    /// The socket is being moved from one form to the other. Never observed by anything
    /// else: it exists because swapping an enum's contents needs something to leave
    /// behind for the moment the old value is owned and the new one is not built yet.
    Taken,
    /// A reading half, and a task that owns the writing half.
    Split {
        reader: ConnReader,
        writer: writer::Writer,
    },
}

impl Wire {
    /// Takes an `Arc` so that neither path copies the message: the whole form borrows it,
    /// the split form moves the handle into the queue.
    pub(super) async fn send(&mut self, msg: std::sync::Arc<Message>) -> ResultType<()> {
        match self {
            Wire::Whole(s) => s.send(&*msg).await,
            Wire::Split { writer, .. } => {
                writer.send(msg);
                Ok(())
            }
            Wire::Taken => Ok(()),
        }
    }

    /// A video message, or the `SwitchDisplay` that must stay ordered with it.
    pub(super) async fn send_video(
        &mut self,
        at: std::time::Instant,
        msg: std::sync::Arc<Message>,
    ) -> ResultType<()> {
        match self {
            Wire::Whole(s) => s.send(&*msg).await,
            Wire::Split { writer, .. } => {
                writer.send_video(at, msg);
                Ok(())
            }
            Wire::Taken => Ok(()),
        }
    }

    pub(super) async fn send_raw(&mut self, bytes: Vec<u8>) -> ResultType<()> {
        match self {
            Wire::Whole(s) => s.send_raw(bytes).await,
            Wire::Split { writer, .. } => {
                writer.send_raw(bytes);
                Ok(())
            }
            Wire::Taken => Ok(()),
        }
    }

    pub(super) async fn send_bytes(&mut self, bytes: bytes::Bytes) -> ResultType<()> {
        match self {
            Wire::Whole(s) => s.send_bytes(bytes).await,
            Wire::Split { writer, .. } => {
                writer.send_raw(bytes.to_vec());
                Ok(())
            }
            Wire::Taken => Ok(()),
        }
    }

    pub(super) async fn next(&mut self) -> Option<Result<bytes::BytesMut, std::io::Error>> {
        match self {
            Wire::Whole(s) => s.next().await,
            Wire::Split { reader, .. } => reader.next().await,
            // Nothing holds this variant across an await point; answering "the peer is
            // gone" is the safe reading if anything ever does.
            Wire::Taken => None,
        }
    }

    pub(super) fn set_send_timeout(&mut self, ms: u64) {
        match self {
            Wire::Whole(s) => s.set_send_timeout(ms),
            Wire::Split { writer, .. } => writer.set_send_timeout(ms),
            Wire::Taken => {}
        }
    }

    pub(super) fn set_raw(&mut self) {
        match self {
            Wire::Whole(s) => s.set_raw(),
            // Raw mode belongs to port forwarding, which is never split.
            Wire::Split { .. } | Wire::Taken => {}
        }
    }

    /// The socket itself, for the two places that still need the whole thing: the file
    /// transfer helper and the port forward's packet-size cap. `None` once the writer task
    /// owns the writing half, and each caller says what it does then.
    pub(super) fn whole(&mut self) -> Option<&mut Stream> {
        match self {
            Wire::Whole(s) => Some(s),
            Wire::Split { .. } | Wire::Taken => None,
        }
    }

    /// Whether the writer task has given up on the socket.
    pub(super) fn is_closed(&self) -> bool {
        match self {
            Wire::Split { writer, .. } => writer.is_closed(),
            _ => false,
        }
    }

    pub(super) fn writer(&self) -> Option<&writer::Writer> {
        match self {
            Wire::Split { writer, .. } => Some(writer),
            _ => None,
        }
    }
}
