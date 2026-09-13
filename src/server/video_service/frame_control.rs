use super::*;

#[inline]
pub fn notify_video_frame_fetched(display_idx: usize, conn_id: i32, frame_tm: Option<Instant>) {
    if let Some(notifier) = FRAME_FETCHED_NOTIFIERS.lock().unwrap().get(&display_idx) {
        notifier.0.send((conn_id, frame_tm)).ok();
    }
}

#[inline]
pub fn notify_video_frame_fetched_by_conn_id(conn_id: i32, frame_tm: Option<Instant>) {
    let vec_display_idx: Vec<usize> = {
        let display_conn_ids = DISPLAY_CONN_IDS.lock().unwrap();
        display_conn_ids
            .iter()
            .filter_map(|(display_idx, conn_ids)| {
                if conn_ids.contains(&conn_id) {
                    Some(*display_idx)
                } else {
                    None
                }
            })
            .collect()
    };
    let notifiers = FRAME_FETCHED_NOTIFIERS.lock().unwrap();
    for display_idx in vec_display_idx {
        if let Some(notifier) = notifiers.get(&display_idx) {
            notifier.0.send((conn_id, frame_tm)).ok();
        }
    }
}

pub(super) struct VideoFrameController {
    pub(super) display_idx: usize,
    pub(super) cur: Instant,
    pub(super) send_conn_ids: HashSet<i32>,
}

impl VideoFrameController {
    pub(super) fn new(display_idx: usize) -> Self {
        Self {
            display_idx,
            cur: Instant::now(),
            send_conn_ids: HashSet::new(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.send_conn_ids.clear();
    }

    pub(super) fn set_send(&mut self, tm: Instant, conn_ids: HashSet<i32>) {
        if !conn_ids.is_empty() {
            self.cur = tm;
            self.send_conn_ids = conn_ids;
            DISPLAY_CONN_IDS
                .lock()
                .unwrap()
                .insert(self.display_idx, self.send_conn_ids.clone());
        }
    }

    /// Block for up to `timeout_millis` until one connection reports the current frame
    /// fetched, then drain whatever else has arrived. A timeout adds nothing.
    pub(super) fn try_wait_next(&mut self, fetched_conn_ids: &mut HashSet<i32>, timeout_millis: u64) {
        if self.send_conn_ids.is_empty() {
            return;
        }

        let receiver = {
            match FRAME_FETCHED_NOTIFIERS
                .lock()
                .unwrap()
                .get(&self.display_idx)
            {
                Some(notifier) => notifier.1.clone(),
                None => {
                    return;
                }
            }
        };
        let receiver = receiver.lock().unwrap();
        let mut note = |(id, instant): (i32, Option<Instant>)| {
            if let Some(tm) = instant {
                log::trace!("Channel recv latency: {}", tm.elapsed().as_secs_f32());
            }
            fetched_conn_ids.insert(id);
        };
        if let Ok(first) = receiver.recv_timeout(Duration::from_millis(timeout_millis)) {
            note(first);
        }
        while let Ok(next) = receiver.try_recv() {
            note(next);
        }
    }
}

/// The capture loop's wait for the connections to pick up the frame it just sent, paced
/// by the link rather than a fixed three seconds: two frame periods, or three round trips
/// when the path is slower than that, within a slice.
pub(super) fn ack_wait_window(spf: Duration, rtt_ms: Option<u32>) -> Duration {
    let rtt = Duration::from_millis(rtt_ms.unwrap_or(0) as u64 * 3);
    (spf * 2).max(rtt).clamp(ACK_WAIT_MIN, ACK_WAIT_SLICE)
}

const ACK_WAIT_MIN: Duration = Duration::from_millis(50);
const ACK_WAIT_SLICE: Duration = Duration::from_millis(300);
/// The old fixed window: a viewer that never fetches must not freeze the others for longer.
const HOLD_LIMIT: Duration = Duration::from_millis(3_000);

/// Keeps the loop from encoding the next frame while the previous one is still unfetched.
/// While holding, the loop keeps capturing (so the next frame is the newest screen) but
/// skips the encode, instead of queueing frames behind a blocked write.
pub(super) struct FetchHold {
    since: Option<Instant>,
    limit: Duration,
    fetched: HashSet<i32>,
    last_wait_ms: u32,
}

impl FetchHold {
    pub(super) fn new() -> Self {
        Self::with_limit(HOLD_LIMIT)
    }

    pub(super) fn with_limit(limit: Duration) -> Self {
        Self {
            since: None,
            limit,
            fetched: HashSet::new(),
            last_wait_ms: 0,
        }
    }

    /// How long the last frame took to be fetched by every connection, for diagnostics.
    pub(super) fn last_wait_ms(&self) -> u32 {
        self.last_wait_ms
    }

    fn all_fetched(&self, fc: &VideoFrameController) -> bool {
        self.fetched.len() >= fc.send_conn_ids.len()
    }

    /// Wait up to `window` in slices, running `tick` between them, for the outstanding fetches.
    fn wait(
        &mut self,
        fc: &mut VideoFrameController,
        window: Duration,
        tick: &mut dyn FnMut() -> ResultType<()>,
    ) -> ResultType<bool> {
        let begin = Instant::now();
        while !self.all_fetched(fc) {
            let left = window.saturating_sub(begin.elapsed());
            if left.is_zero() {
                break;
            }
            tick()?;
            fc.try_wait_next(&mut self.fetched, left.min(ACK_WAIT_SLICE).as_millis() as u64);
        }
        Ok(self.all_fetched(fc))
    }

    /// A frame just went out: wait `window` for its fetches, and hold if some are missing.
    pub(super) fn after_send(
        &mut self,
        fc: &mut VideoFrameController,
        window: Duration,
        mut tick: impl FnMut() -> ResultType<()>,
    ) -> ResultType<()> {
        self.fetched.clear();
        let begin = Instant::now();
        if self.wait(fc, window, &mut tick)? {
            self.resolve(fc, begin.elapsed());
        } else {
            self.since = Some(begin);
        }
        Ok(())
    }

    /// Before the next encode: `true` when every connection has the previous frame, or the
    /// hold has lasted `limit`; `false` to skip this round (it already waited `window`).
    pub(super) fn may_encode(
        &mut self,
        fc: &mut VideoFrameController,
        window: Duration,
        mut tick: impl FnMut() -> ResultType<()>,
    ) -> ResultType<bool> {
        let Some(since) = self.since else {
            return Ok(true);
        };
        if self.wait(fc, window, &mut tick)? || since.elapsed() >= self.limit {
            self.resolve(fc, since.elapsed());
            return Ok(true);
        }
        Ok(false)
    }

    fn resolve(&mut self, fc: &VideoFrameController, waited: Duration) {
        self.since = None;
        self.last_wait_ms = waited.as_millis() as u32;
        DISPLAY_CONN_IDS.lock().unwrap().remove(&fc.display_idx);
    }
}
