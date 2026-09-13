//! What is waiting to go to the peer, and what happens when it piles up.

use base::message_proto::Message;
use std::{collections::VecDeque, sync::Arc, time::Instant};

/// How many video messages may wait. A frame is worth sending only while it is still
/// roughly current; beyond about a second of backlog the peer would be shown a slideshow
/// of the past, so the queue is short on purpose and the oldest is what goes.
///
/// **Reasoned, not measured.** Thirty is about a second at the frame rates we see, which
/// is the point where a frame stops being worth its place. Nothing has been run on a link
/// thin enough to fill this queue, so treat it as a starting value to calibrate rather
/// than a verified constant; `docs/backlog.md` says what that calibration needs.
pub(super) const VIDEO_QUEUE_CAP: usize = 30;

/// One thing to write. Raw bytes are messages another process already encoded; they go
/// out as they are.
pub(super) enum Out {
    Msg(Arc<Message>),
    Raw(Vec<u8>),
}

pub(super) type Item = (Instant, Out);

#[derive(Default)]
pub(super) struct Queues {
    /// Everything that is not video. Never bounded and never dropped: losing one of these
    /// does not degrade the picture, it makes the session behave wrongly -- a permission
    /// that never arrives, a clipboard that never updates, a close reason never seen.
    control: VecDeque<Item>,
    /// Video frames, and `SwitchDisplay` with them. **`SwitchDisplay` belongs in this
    /// queue and nowhere else**: it has to reach the peer after the last frame of the old
    /// display and before the first frame of the new one, so a decoder is never handed a
    /// frame with the wrong parameters. Anything that jumps it ahead of the video breaks
    /// exactly the case it exists for.
    video: VecDeque<Item>,
    /// Video messages thrown away because the link could not keep up. Counted rather than
    /// logged per occurrence, so that a slow link does not also produce a flood of logs.
    dropped: u64,
    closed: bool,
}

impl Queues {
    pub(super) fn push_control(&mut self, item: Item) {
        self.control.push_back(item);
    }

    /// Adds a video message, discarding the oldest when the queue is full.
    ///
    /// Dropping the oldest rather than refusing the newest is deliberate: the newest frame
    /// is the one that resembles what the user is looking at.
    pub(super) fn push_video(&mut self, item: Item) {
        if self.video.len() >= VIDEO_QUEUE_CAP {
            self.video.pop_front();
            self.dropped += 1;
        }
        self.video.push_back(item);
    }

    /// The next thing to write: everything on the control queue before any video.
    ///
    /// This is where a control message overtakes a video backlog, and it is safe precisely
    /// because `SwitchDisplay` is not on this queue. Each queue keeps its own order; the
    /// two are unordered with respect to each other, which is the same guarantee the
    /// connection loop gave when it selected over two channels.
    pub(super) fn pop(&mut self) -> Option<Item> {
        self.control.pop_front().or_else(|| self.video.pop_front())
    }

    pub(super) fn video_len(&self) -> usize {
        self.video.len()
    }

    /// Reads the drop counter and clears it, so a caller can react once per period.
    pub(super) fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }

    pub(super) fn close(&mut self) {
        self.closed = true;
    }

    pub(super) fn is_closed(&self) -> bool {
        self.closed
    }
}
