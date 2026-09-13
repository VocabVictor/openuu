//! The queue between the network thread and the decode thread.
//!
//! It used to be a fixed 120-slot FIFO, which at 30 fps is four seconds of
//! video: when decoding falls behind, every queued frame is still decoded
//! and displayed, so the picture keeps drifting further behind
//! (docs/perf-review.md §3.4). The queue now holds about half a second of
//! frames, whatever fps was negotiated, and drops the oldest ones when it
//! has to.
//!
//! Only a contiguous run at the front is ever dropped: skipping a frame in
//! the middle and decoding the ones behind it would feed the decoder
//! references it never saw. A drop still breaks the reference chain, so
//! `push` reports it and the caller asks the peer for a key frame.

use base::message_proto::VideoFrame;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

/// Hard ceiling of the queue. The effective limit is dynamic and well below
/// it; this only bounds what a burst can take.
pub const VIDEO_QUEUE_SIZE: usize = 120;
/// How much video the queue is allowed to hold.
const QUEUE_MS: usize = 500;
/// Even at 1 fps a frame may arrive while one is being decoded.
const MIN_LIMIT: usize = 2;
const DEFAULT_FPS: usize = 30;

/// How many frames are about [`QUEUE_MS`] at `fps`.
pub fn limit_for_fps(fps: usize) -> usize {
    let fps = if fps == 0 { DEFAULT_FPS } else { fps };
    (fps * QUEUE_MS / 1000).clamp(MIN_LIMIT, VIDEO_QUEUE_SIZE)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PushOutcome {
    /// Frames dropped from the front to stay within the limit.
    pub dropped: usize,
}

impl PushOutcome {
    /// Dropping a frame leaves the decoder without the references the
    /// frames behind it were coded against, so the caller has to ask the
    /// peer for a key frame.
    pub fn needs_key_frame(&self) -> bool {
        self.dropped > 0
    }
}

pub struct VideoFrameQueue {
    queue: Mutex<VecDeque<VideoFrame>>,
    limit: AtomicUsize,
}

impl VideoFrameQueue {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::with_capacity(limit_for_fps(DEFAULT_FPS))),
            limit: AtomicUsize::new(limit_for_fps(DEFAULT_FPS)),
        }
    }

    /// Follows the fps the peer is sending at, so the queue keeps holding
    /// about half a second whatever that fps is.
    pub fn set_target_fps(&self, fps: usize) {
        self.limit.store(limit_for_fps(fps), Ordering::Relaxed);
    }

    pub fn limit(&self) -> usize {
        self.limit.load(Ordering::Relaxed)
    }

    /// The hard ceiling, not the dynamic limit: callers that reason about
    /// the worst case (fps control) want this one.
    pub fn capacity(&self) -> usize {
        VIDEO_QUEUE_SIZE
    }

    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }

    /// Appends `vf` and drops the oldest frames to stay within the limit.
    pub fn push(&self, vf: VideoFrame) -> PushOutcome {
        let limit = self.limit().min(VIDEO_QUEUE_SIZE).max(1);
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(vf);
        let dropped = queue.len().saturating_sub(limit);
        queue.drain(..dropped);
        PushOutcome { dropped }
    }

    pub fn pop(&self) -> Option<VideoFrame> {
        self.queue.lock().unwrap().pop_front()
    }

    /// Forgets what is queued. Called when a key frame arrives outside the
    /// queue: it starts a new reference chain, so the frames before it are
    /// both undecodable and older than what is about to be shown.
    pub fn clear(&self) {
        self.queue.lock().unwrap().clear();
    }
}

impl Default for VideoFrameQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // VideoFrame carries no timestamp, so the tests number the frames
    // through `display` to follow which ones survive.
    fn frame(id: i32) -> VideoFrame {
        VideoFrame {
            display: id,
            ..Default::default()
        }
    }

    fn drain(q: &VideoFrameQueue) -> Vec<i32> {
        let mut out = Vec::new();
        while let Some(vf) = q.pop() {
            out.push(vf.display);
        }
        out
    }

    #[test]
    fn the_limit_is_half_a_second_of_frames() {
        assert_eq!(limit_for_fps(30), 15);
        assert_eq!(limit_for_fps(60), 30);
        assert_eq!(limit_for_fps(120), 60);
        // Degenerate fps values still leave room for one frame in flight.
        assert_eq!(limit_for_fps(1), MIN_LIMIT);
        assert_eq!(limit_for_fps(0), limit_for_fps(DEFAULT_FPS));
        assert!(limit_for_fps(usize::MAX / 1000) <= VIDEO_QUEUE_SIZE);
    }

    #[test]
    fn pushes_below_the_limit_drop_nothing() {
        let q = VideoFrameQueue::new();
        q.set_target_fps(30);
        for i in 0..q.limit() as i32 {
            assert!(!q.push(frame(i)).needs_key_frame());
        }
        assert_eq!(q.len(), 15);
        assert_eq!(drain(&q), (0..15).collect::<Vec<_>>());
    }

    #[test]
    fn a_full_queue_drops_the_oldest_frame() {
        let q = VideoFrameQueue::new();
        q.set_target_fps(30);
        for i in 0..15 {
            q.push(frame(i));
        }
        let outcome = q.push(frame(15));
        assert_eq!(outcome.dropped, 1);
        assert!(outcome.needs_key_frame());
        let kept = drain(&q);
        assert_eq!(kept.len(), 15);
        assert_eq!(kept.first(), Some(&1), "the oldest frame went");
        assert_eq!(kept.last(), Some(&15), "the newest one stayed");
    }

    #[test]
    fn lowering_the_fps_trims_the_queue_on_the_next_push() {
        let q = VideoFrameQueue::new();
        q.set_target_fps(60);
        for i in 0..30 {
            q.push(frame(i));
        }
        q.set_target_fps(10);
        assert_eq!(q.limit(), 5);
        assert_eq!(q.push(frame(30)).dropped, 26, "26 over the new limit");
        assert_eq!(drain(&q), (26..=30).collect::<Vec<_>>());
    }

    #[test]
    fn a_key_frame_makes_the_queued_frames_obsolete() {
        let q = VideoFrameQueue::new();
        for i in 0..5 {
            q.push(frame(i));
        }
        q.clear();
        assert!(q.is_empty());
        assert!(q.pop().is_none());
    }

    /// A decoder that cannot keep up: the queue stays within half a second,
    /// what it drops is a run at the front, and the frame being decoded is
    /// never further behind than the limit.
    #[test]
    fn a_slow_decoder_keeps_the_backlog_bounded_and_in_order() {
        let q = VideoFrameQueue::new();
        q.set_target_fps(30);
        let mut dropped = 0;
        let mut decoded: Vec<i32> = Vec::new();
        for i in 0..300 {
            dropped += q.push(frame(i)).dropped;
            assert!(q.len() <= q.limit(), "over the limit at frame {i}");
            // The decoder drains one frame for every two that arrive.
            if i % 2 == 0 {
                if let Some(vf) = q.pop() {
                    if let Some(last) = decoded.last() {
                        assert!(vf.display > *last, "frames came back out of order");
                    }
                    decoded.push(vf.display);
                }
            }
            if let Some(last) = decoded.last() {
                assert!(
                    i - last <= q.limit() as i32 + 1,
                    "frame {last} is {} behind the newest",
                    i - last
                );
            }
        }
        assert!(dropped > 0, "a slow decoder must have lost frames");
        assert!(decoded.len() < 300);
    }

    /// With the decoder keeping up, nothing is dropped and every frame is
    /// seen in order.
    #[test]
    fn a_decoder_that_keeps_up_loses_nothing() {
        let q = VideoFrameQueue::new();
        q.set_target_fps(30);
        let mut decoded = Vec::new();
        for i in 0..300 {
            assert_eq!(q.push(frame(i)).dropped, 0);
            while let Some(vf) = q.pop() {
                decoded.push(vf.display);
            }
        }
        assert_eq!(decoded, (0..300).collect::<Vec<_>>());
    }
}
