use super::*;

#[derive(Default, Debug, Clone)]
pub(super) struct UserDelay {
    pub(super) stall_ticks: u8, // timer ticks the outstanding probe has been out beyond two seconds
    pub(super) delay_history: VecDeque<u32>,
    pub(super) fps: Option<u32>,
    pub(super) rtt_calculator: RttCalculator,
    pub(super) quick_increase_fps_count: usize,
    pub(super) increase_fps_count: usize,
    pub(super) consecutive_bad_samples: usize,
    pub(super) fps_bad_samples: u8, // fresh bad replies since the last FPS reduction
    pub(super) good_samples: usize, // since the last reduction, capped at 3
    pub(super) replies_after_bitrate_reduction: Option<u8>,
    pub(super) fps_before_congestion: Option<u32>, // level to return to once replies are good again
    pub(super) samples_since_restore: Option<u8>,  // set by a restore, cleared once it proved stable
    pub(super) stall_reference_fps: Option<u32>,   // fps when the outstanding probe passed two seconds
    pub(super) startup_good_samples: u8,           // u8::MAX permanently ends startup acceleration
}

impl UserDelay {
    pub(super) fn add_delay(&mut self, delay: u32) {
        if self.delay_history.len() >= HISTORY_DELAY_LEN {
            self.delay_history.pop_front();
        }
        self.delay_history.push_back(delay);
    }

    pub(super) fn limit_fps_change(
        &mut self,
        current_fps: u32,
        fps: u32,
        delay: u32,
        bitrate_first: bool,
        braked: bool,
    ) -> u32 {
        // A spike stays in the average for several samples; confirm congestion with fresh samples.
        let delay = delay.saturating_sub(self.rtt_calculator.get_rtt().unwrap_or_default());
        if let Some(samples) = self.samples_since_restore.as_mut() {
            *samples = samples.saturating_add(1);
        }
        if delay < DELAY_THRESHOLD_150MS {
            self.consecutive_bad_samples = 0;
            self.fps_bad_samples = 0;
            self.replies_after_bitrate_reduction = None;
            self.good_samples = (self.good_samples + 1).min(3);
            return self.recover(current_fps, fps);
        }
        self.consecutive_bad_samples = (self.consecutive_bad_samples + 1).min(3);
        self.fps_bad_samples = (self.fps_bad_samples + 1).min(3);
        if let Some(replies) = self.replies_after_bitrate_reduction.as_mut() {
            *replies = (*replies + 1).min(2);
        }
        let failed_restore = delay >= 600
            && self
                .samples_since_restore
                .is_some_and(|samples| samples <= RESTORE_GUARD_SAMPLES);
        // A level that congests right after being restored is not the level to return to.
        if self
            .samples_since_restore
            .is_some_and(|samples| samples <= RESTORE_GUARD_SAMPLES)
            && (failed_restore || self.consecutive_bad_samples >= 3)
        {
            self.fps_before_congestion = Some(current_fps - current_fps / 4);
            self.samples_since_restore = None;
        }
        // The timeout brake already reduced for the probe this reply answers.
        if fps >= current_fps || braked {
            return current_fps;
        }
        // An extra second of delay cannot wait for another confirmation.
        if !failed_restore
            && delay < 1000
            && (self.fps_bad_samples < 3
                || (bitrate_first && self.replies_after_bitrate_reduction.unwrap_or_default() < 2))
        {
            return current_fps;
        }
        // A fast restore probes capacity. Roll it back promptly if the queue grows
        // again, rather than waiting through another ordinary confirmation window.
        let divisor = if delay >= 1000 || failed_restore {
            2
        } else {
            5
        };
        self.on_reduction(current_fps);
        fps.max(current_fps.saturating_sub((current_fps / divisor).max(1)))
    }

    // Fresh low-delay replies permit recovery even while the average contains a spike:
    // a little at first, then back to the level held before congestion.
    pub(super) fn recover(&mut self, current_fps: u32, fps: u32) -> u32 {
        let gradual = current_fps + (current_fps / 10).max(1);
        let level = self
            .fps_before_congestion
            .filter(|level| *level > current_fps);
        match (self.good_samples, level) {
            (2 | 3, Some(level)) => {
                self.fps_before_congestion = None;
                self.samples_since_restore = Some(0);
                fps.max(level)
            }
            (3, None) => {
                self.fps_before_congestion = None;
                fps.max(current_fps + (current_fps / 5).max(2))
            }
            _ => gradual,
        }
    }

    pub(super) fn accelerate_startup(
        &mut self,
        current_fps: u32,
        fps: u32,
        cap: u32,
        delay: u32,
        braked: bool,
    ) -> u32 {
        if self.startup_good_samples == u8::MAX {
            return fps;
        }
        let excess = delay.saturating_sub(self.rtt_calculator.get_rtt().unwrap_or_default());
        // A low-load sample does not establish capacity: require two clean replies
        // per step and abandon startup probing on the first sign of queue growth.
        if braked || excess >= 50 || current_fps >= cap {
            self.startup_good_samples = u8::MAX;
            return fps;
        }
        self.startup_good_samples += 1;
        if self.startup_good_samples < 2 {
            return fps;
        }
        self.startup_good_samples = 0;
        let accelerated = fps.max(current_fps.saturating_mul(2)).min(cap);
        if accelerated >= cap {
            self.startup_good_samples = u8::MAX;
        }
        accelerated
    }

    // The first reduction of an episode remembers the level to return to.
    pub(super) fn on_reduction(&mut self, current_fps: u32) {
        self.startup_good_samples = u8::MAX;
        self.good_samples = 0;
        self.fps_bad_samples = 0;
        if self.fps_before_congestion.is_none() {
            self.fps_before_congestion = Some(current_fps);
        }
    }

    // Bitrate is cut on confirmation only: two bad replies in a row, or a probe still
    // outstanding at the second tick past two seconds.  One slow reply or one short
    // stall is jitter, and a static screen would never earn the cut back.
    pub(super) fn needs_bitrate_reduction(&self) -> bool {
        self.consecutive_bad_samples >= 2 || self.stall_ticks >= 2
    }

    // The bitrate step this viewer's own evidence calls for, None when it calls for
    // none.  Severity and confirmation come from the same viewer; the controller
    // never pairs one viewer's spike with another viewer's confirmation.
    pub(super) fn ratio_reduction(&self) -> Option<f32> {
        if !self.needs_bitrate_reduction() {
            return None;
        }
        let excess = self.avg_delay();
        let confirmed = self.consecutive_bad_samples >= 3;
        Some(if excess < 200 {
            0.95
        } else if excess < 300 {
            0.9
        } else if excess < 500 {
            if confirmed {
                0.7
            } else {
                0.85
            }
        } else if confirmed {
            0.5
        } else {
            0.8
        })
    }

    // Average delay above the baseline: what the queue adds on top of the path itself.
    pub(super) fn avg_delay(&self) -> u32 {
        if self.delay_history.is_empty() {
            return DELAY_THRESHOLD_150MS;
        }
        let avg_delay = self.delay_history.iter().sum::<u32>() / self.delay_history.len() as u32;
        avg_delay.saturating_sub(self.rtt_calculator.get_rtt().unwrap_or_default())
    }
}
