//! The task that owns the write half of a connection.
//!
//! # What it is for
//!
//! While the connection loop owned the socket, a write that blocked held the whole
//! `select!`, so input, clipboard and probe replies waited behind the picture — measured
//! at up to 1.4 s on a 2 Mbps link. With the write in its own task the loop never waits on
//! the socket, and a control message overtakes a video backlog instead of queuing behind
//! it.
//!
//! # The ordering it guarantees, and the one it must not
//!
//! Two queues. Control messages in one, video **and `SwitchDisplay`** in the other. Each
//! queue is strictly ordered; the two are unordered with respect to each other, which is
//! the same guarantee the loop gave when it selected over two channels.
//!
//! `SwitchDisplay` travelling with the video is not an accident to be tidied up. It has to
//! arrive after the last frame of the old display and before the first frame of the new
//! one, or the peer decodes frames with the wrong parameters. `ConnInner::send` routes it
//! to the video channel for this reason and has done so since before this task existed.
//! **Do not give it a fast path.**
//!
//! # What it measures
//!
//! The time a video write spends blocked is the evidence the bitrate controller uses to
//! size the link (`VideoQoS::note_link_capacity`). That measurement has to live where the
//! write happens, so it moved in here with it. Losing it would not fail anything visibly:
//! the controller would simply go blind on thin links again.

use super::*;
use crate::stream_split::ConnWriter;
use hbb_common::{
    message_proto::Message,
    tokio::{self, sync::Notify},
};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

mod queue;
use queue::Queues;

#[cfg(test)]
mod tests;

/// What the writer did since the last time anyone asked. Read once a second by the
/// connection loop, which owns the reaction: feeding the bitrate controller and asking for
/// a fresh key frame after a drop.
#[derive(Default, Debug, PartialEq)]
pub(super) struct SendReport {
    /// Bits of video actually handed to the socket.
    pub bits: u64,
    /// Total milliseconds video writes spent blocked.
    pub blocked_ms: u32,
    /// The worst single write.
    pub max_ms: u32,
    /// Video messages written.
    pub count: u32,
    /// Video messages discarded because the queue was full.
    pub dropped: u64,
    /// Video messages still waiting.
    pub queued: usize,
}

#[derive(Default)]
struct Totals {
    bits: u64,
    blocked_ms: u32,
    max_ms: u32,
    count: u32,
}

struct Shared {
    queues: Mutex<Queues>,
    totals: Mutex<Totals>,
    wake: Notify,
}

/// The connection loop's end of the writer. Every method returns immediately; nothing here
/// can block on the socket, which is the whole point.
pub(super) struct Writer {
    shared: Arc<Shared>,
}

impl Writer {
    /// Starts the task and returns the handle. The task ends when `close` is called or the
    /// socket fails.
    pub(super) fn start(mut out: ConnWriter) -> Self {
        let shared = Arc::new(Shared {
            queues: Default::default(),
            totals: Default::default(),
            wake: Notify::new(),
        });
        let task = shared.clone();
        tokio::spawn(async move {
            loop {
                let next = {
                    let mut q = task.queues.lock().unwrap();
                    match q.pop() {
                        Some(item) => Some(item),
                        None if q.is_closed() => break,
                        None => None,
                    }
                };
                let Some((_instant, msg)) = next else {
                    task.wake.notified().await;
                    continue;
                };
                let is_video = matches!(msg.union, Some(message::Union::VideoFrame(_)));
                let bits = if is_video { 8 * msg.compute_size() } else { 0 };
                let began = Instant::now();
                if out.send(&msg as &Message).await.is_err() {
                    task.queues.lock().unwrap().close();
                    break;
                }
                if is_video {
                    let blocked = began.elapsed().as_millis() as u32;
                    let mut t = task.totals.lock().unwrap();
                    t.bits = t.bits.saturating_add(bits);
                    t.blocked_ms = t.blocked_ms.saturating_add(blocked);
                    t.max_ms = t.max_ms.max(blocked);
                    t.count += 1;
                }
            }
        });
        Self { shared }
    }

    /// Queues anything that is not video. Never dropped.
    pub(super) fn send(&self, msg: Arc<Message>) {
        self.shared
            .queues
            .lock()
            .unwrap()
            .push_control((Instant::now(), msg));
        self.shared.wake.notify_one();
    }

    /// Queues a video message, or a `SwitchDisplay` that must stay ordered with the video.
    /// The oldest goes when the queue is full.
    pub(super) fn send_video(&self, at: Instant, msg: Arc<Message>) {
        self.shared.queues.lock().unwrap().push_video((at, msg));
        self.shared.wake.notify_one();
    }

    /// What happened since the last call, with the counters cleared.
    pub(super) fn take_report(&self) -> SendReport {
        let (dropped, queued) = {
            let mut q = self.shared.queues.lock().unwrap();
            (q.take_dropped(), q.video_len())
        };
        let t = std::mem::take(&mut *self.shared.totals.lock().unwrap());
        SendReport {
            bits: t.bits,
            blocked_ms: t.blocked_ms,
            max_ms: t.max_ms,
            count: t.count,
            dropped,
            queued,
        }
    }

    /// True once the socket has failed; the loop ends the session on it.
    pub(super) fn is_closed(&self) -> bool {
        self.shared.queues.lock().unwrap().is_closed()
    }

    pub(super) fn close(&self) {
        self.shared.queues.lock().unwrap().close();
        self.shared.wake.notify_one();
    }
}
