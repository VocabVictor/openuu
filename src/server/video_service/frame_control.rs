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
