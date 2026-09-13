use super::*;

// Common adjust functions
impl VideoQoS {
    pub fn new_display(&mut self, video_service_name: String) {
        self.displays
            .insert(video_service_name, DisplayData::default());
    }

    pub fn remove_display(&mut self, video_service_name: &str) {
        self.displays.remove(video_service_name);
    }

    pub fn update_display_data(&mut self, video_service_name: &str, send_counter: usize) {
        if let Some(display) = self.displays.get_mut(video_service_name) {
            display.send_counter += send_counter;
        }
        self.adjust_fps();
        let abr_enabled = self.in_vbr_state();
        if abr_enabled {
            if self.since(self.adjust_ratio_instant).as_secs() >= ADJUST_RATIO_INTERVAL as u64 {
                let dynamic_screen = self
                    .displays
                    .iter()
                    .any(|d| d.1.send_counter >= ADJUST_RATIO_INTERVAL * DYNAMIC_SCREEN_THRESHOLD);
                self.adjust_ratio(dynamic_screen);
            }
        } else {
            self.ratio = self.latest_quality().ratio();
        }
    }

    #[inline]
    pub(super) fn highest_fps(&self) -> u32 {
        self.users
            .values()
            .map(|u| u.fps_cap())
            .min()
            .unwrap_or(FPS)
            .clamp(MIN_FPS, MAX_FPS)
    }

    // Get latest quality settings from all users
    pub fn latest_quality(&self) -> Quality {
        self.users
            .iter()
            .map(|(_, u)| u.quality)
            .filter(|q| *q != None)
            .max_by(|a, b| a.unwrap_or_default().0.cmp(&b.unwrap_or_default().0))
            .flatten()
            .unwrap_or((0, Quality::Balanced))
            .1
    }

    // Lowest ratio the latest quality allows: keeps about 1Mbps at high resolutions.
    pub(super) fn min_ratio(&self) -> f32 {
        let current_bitrate = self.bitrate();
        let ratio_1mbps = if current_bitrate > 0 {
            Some((self.ratio * 1000.0 / current_bitrate as f32).max(BR_MIN_HIGH_RESOLUTION))
        } else {
            None
        };
        match self.latest_quality() {
            Quality::Best => {
                let mut min = BR_BEST / 2.5;
                if let Some(ratio_1mbps) = ratio_1mbps {
                    if min > ratio_1mbps {
                        min = ratio_1mbps;
                    }
                }
                min.max(BR_MIN)
            }
            Quality::Balanced => {
                let mut min = (BR_BALANCED / 2.0).min(0.4);
                if let Some(ratio_1mbps) = ratio_1mbps {
                    if min > ratio_1mbps {
                        min = ratio_1mbps;
                    }
                }
                min.max(BR_MIN_HIGH_RESOLUTION)
            }
            Quality::Low | Quality::Custom(_) => BR_MIN_HIGH_RESOLUTION,
        }
    }

    // Whether congestion can still be answered with a lower bitrate.  Within two
    // percent of the floor another step is not worth waiting a cooldown for.
    pub(super) fn can_reduce_bitrate(&self) -> bool {
        self.in_vbr_state() && !self.displays.is_empty() && self.ratio > self.min_ratio() * 1.02
    }

    /// What the send path pushed out over a second it spent blocked on the link. Before
    /// the first probe comes back this is the only evidence about the link there is, and
    /// a session that opens at a preset the link cannot carry spends those seconds filling
    /// a queue it then has to drain. A lower bound, acted on only downwards.
    pub fn note_link_capacity(&mut self, kbps: u32) {
        if kbps == 0 || !self.in_vbr_state() {
            return;
        }
        let current = self.bitrate();
        let floor = self.min_ratio();
        if current == 0 || kbps >= current || self.ratio <= floor {
            return;
        }
        let fit = self.ratio * kbps as f32 / current as f32 * LINK_FIT;
        let next = fit.clamp(floor, self.ratio);
        if next >= self.ratio {
            return;
        }
        log::debug!(
            "qos_trace t={} link={kbps} bitrate={current} ratio={:.3}->{next:.3}",
            hbb_common::get_time(),
            self.ratio,
        );
        self.ratio = next;
        self.reset_send_counters();
        self.adjust_ratio_instant = self.now();
    }

    // A viewer whose queue has to be drained is not on the cooldown: the sooner the
    // bitrate goes under what the link carries, the less backlog there is to drain.
    pub(super) fn ratio_adjust_allowed(&self) -> bool {
        self.users.values().any(|u| u.delay.draining())
            || self.since(self.adjust_ratio_instant).as_secs() >= ADJUST_RATIO_INTERVAL as u64
    }

    // Every ratio adjustment starts a new window for the dynamic screen counters.
    pub(super) fn reset_send_counters(&mut self) {
        self.displays.values_mut().for_each(|d| d.send_counter = 0);
    }

    // Adjust quality ratio based on network delay and screen changes
    pub(super) fn adjust_ratio(&mut self, dynamic_screen: bool) {
        if !self.in_vbr_state() {
            return;
        }
        // Get maximum delay from all users
        let max_delay = self.users.iter().map(|u| u.1.delay.avg_delay()).max();
        let Some(max_delay) = max_delay else {
            return;
        };
        // Each viewer judges its own delay; the stream takes the most conservative
        // step any viewer asks for.
        let reduction = self
            .users
            .values()
            .filter_map(|u| u.delay.ratio_reduction())
            .reduce(f32::min);
        if reduction.is_none() && max_delay >= DELAY_THRESHOLD_150MS {
            // Elevated but unconfirmed: no change, and no cooldown either, so a
            // confirmation on the next reply is acted on at once.
            self.reset_send_counters();
            return;
        }

        let target_ratio = self.latest_quality().ratio();
        let current_ratio = self.ratio;
        // A backlog is drained, not stepped away from: go under the link and stay there
        // until the queue has gone.  The clamp lifts the ratio back to the ordinary floor
        // on the first adjustment after that.
        let draining = self.users.values().any(|u| u.delay.draining());

        let min = if draining {
            BR_MIN_DRAIN
        } else {
            self.min_ratio()
        };
        let max = target_ratio * MAX_BR_MULTIPLE;

        let mut v = current_ratio;

        // Three bad replies in a row confirm congestion; with a bitrate-targeted
        // encoder the bitrate is then the only thing that drains the queue, so it
        // comes down hard.  Increases need every viewer below the threshold and
        // compound: 15 percent a window brings a quartered bitrate back in ten
        // windows, where a fixed 150 kbps step took half a minute longer.
        if let Some(factor) = reduction {
            v = current_ratio * factor;
        } else if max_delay < 50 {
            if dynamic_screen {
                v = current_ratio * 1.15;
            }
        } else if max_delay < 100 {
            if dynamic_screen {
                v = current_ratio * 1.1;
            }
        } else if dynamic_screen {
            v = current_ratio * 1.05;
        }

        if reduction.is_some() {
            for user in self.users.values_mut() {
                if user.delay.needs_bitrate_reduction()
                    && user.delay.replies_after_bitrate_reduction.is_none()
                {
                    // One outstanding probe may have started before the bitrate change.
                    user.delay.replies_after_bitrate_reduction =
                        Some(if v.clamp(min, max) < current_ratio {
                            0
                        } else {
                            2
                        });
                }
            }
        }
        // Bad evidence never raises the bitrate: the floor may be above where a drain
        // left it, and lifting it back there is for the first good reply, not this one.
        self.ratio = match reduction {
            Some(_) => v.clamp(min, max).min(current_ratio),
            None => v.clamp(min, max),
        };
        self.reset_send_counters();
        self.adjust_ratio_instant = self.now();
    }

    // Adjust fps based on network delay and user response time
    pub(super) fn adjust_fps(&mut self) {
        let highest_fps = self.highest_fps();
        // Get minimum fps from all users
        let mut fps = self
            .users
            .iter()
            .map(|u| u.1.delay.fps.unwrap_or(INIT_FPS))
            .min()
            .unwrap_or(INIT_FPS);

        // Every viewer inside its first second keeps the stream at INIT_FPS to
        // ensure stability; each viewer carries its own start-up clock.
        if self.users.values().any(|u| {
            u.joined_at
                .is_some_and(|joined| self.since(joined).as_secs() < 1)
        }) {
            fps = fps.min(INIT_FPS);
        }

        // Ensure fps stays within valid range
        self.fps = fps.clamp(MIN_FPS, highest_fps);
    }
}
