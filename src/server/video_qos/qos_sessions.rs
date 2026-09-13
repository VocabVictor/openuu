use super::*;

// User session management
impl VideoQoS {
    // Initialize new user session
    pub fn on_connection_open(&mut self, id: i32) {
        let user = UserData {
            joined_at: Some(self.now()),
            ..Default::default()
        };
        self.users.insert(id, user);
        self.abr_config = Config::get_option("enable-abr") != "N";
    }

    // Clean up user session
    pub fn on_connection_close(&mut self, id: i32) {
        self.users.remove(&id);
        if self.users.is_empty() {
            *self = Default::default();
            return;
        }
        // The stream follows the remaining viewers at once; a departed viewer's
        // start-up guard left with its entry.
        self.adjust_fps();
    }

    pub fn user_custom_fps(&mut self, id: i32, fps: u32) {
        if fps < MIN_FPS || fps > MAX_FPS {
            return;
        }
        if let Some(user) = self.users.get_mut(&id) {
            user.custom_fps = Some(fps);
        }
    }

    pub fn user_auto_adjust_fps(&mut self, id: i32, fps: u32) {
        if fps < MIN_FPS || fps > MAX_FPS {
            return;
        }
        if let Some(user) = self.users.get_mut(&id) {
            user.auto_adjust_fps = Some(fps);
        }
    }

    pub fn user_image_quality(&mut self, id: i32, image_quality: i32) {
        let convert_quality = |q: i32| -> Quality {
            if q == ImageQuality::Balanced.value() {
                Quality::Balanced
            } else if q == ImageQuality::Low.value() {
                Quality::Low
            } else if q == ImageQuality::Best.value() {
                Quality::Best
            } else {
                let b = ((q >> 8 & 0xFFF) * 2) as f32 / 100.0;
                Quality::Custom(b.clamp(BR_MIN, BR_MAX))
            }
        };

        let quality = Some((hbb_common::get_time(), convert_quality(image_quality)));
        if let Some(user) = self.users.get_mut(&id) {
            user.quality = quality;
            // update ratio directly
            self.ratio = self.latest_quality().ratio();
        }
    }

    pub fn user_record(&mut self, id: i32, v: bool) {
        if let Some(user) = self.users.get_mut(&id) {
            user.record = v;
        }
    }

    pub fn user_network_delay(&mut self, id: i32, delay: u32) {
        let target_ratio = self.latest_quality().ratio();
        // Fewer frames only save bytes with encoders that size frames for a fixed rate;
        // bitrate-targeted encoders keep the bitrate, so the bitrate has to come down first.
        let bitrate_first = self.can_reduce_bitrate();

        // For bad network, small fps means quick reaction and high quality
        let (min_fps, normal_fps) = if target_ratio >= BR_BEST {
            (8, 16)
        } else if target_ratio >= BR_BALANCED {
            (10, 20)
        } else {
            (12, 24)
        };

        // Calculate minimum acceptable delay-fps product
        let dividend_ms = DELAY_THRESHOLD_150MS * min_fps;

        let mut adjust_ratio = false;
        let mut reduce_bitrate = false;
        if let Some(user) = self.users.get_mut(&id) {
            let delay = delay.max(10);
            // The reply closes the outstanding probe, braked or not.
            user.delay.stall_ticks = 0;
            user.delay.note_backlog();
            let braked = user.delay.stall_reference_fps.take().is_some();
            let old_avg_delay = user.delay.avg_delay();
            if !braked {
                user.delay.rtt_calculator.update(delay);
            }
            user.delay.add_delay(delay);
            let mut avg_delay = user.delay.avg_delay();
            avg_delay = avg_delay.max(10);
            // Each viewer adapts from its own target, starts at INIT_FPS and is capped
            // by its own limit.  The stream follows the slowest viewer in adjust_fps;
            // neither that minimum nor another viewer's limit feeds back into it.
            let user_cap = user.fps_cap();
            let current_fps = user.delay.fps.unwrap_or(INIT_FPS.min(user_cap));
            let mut fps = current_fps;

            // Adaptive FPS adjustment based on network delay:
            if avg_delay < 50 {
                user.delay.quick_increase_fps_count += 1;
                let mut step = if fps < normal_fps { 1 } else { 0 };
                if user.delay.quick_increase_fps_count >= 3 {
                    // After 3 consecutive good samples, increase more aggressively
                    user.delay.quick_increase_fps_count = 0;
                    step = 5;
                }
                fps = min_fps.max(fps + step);
            } else if avg_delay < 100 {
                let step = if avg_delay < old_avg_delay {
                    if fps < normal_fps {
                        1
                    } else {
                        0
                    }
                } else {
                    0
                };
                fps = min_fps.max(fps + step);
            } else if avg_delay < DELAY_THRESHOLD_150MS {
                fps = min_fps.max(fps);
            } else {
                let devide_fps = ((fps as f32) / (avg_delay as f32 / DELAY_THRESHOLD_150MS as f32))
                    .ceil() as u32;
                if avg_delay < 200 {
                    fps = min_fps.max(devide_fps);
                } else if avg_delay < 300 {
                    fps = min_fps.min(devide_fps);
                } else if avg_delay < 600 {
                    fps = dividend_ms / avg_delay;
                } else {
                    fps = (dividend_ms / avg_delay).min(devide_fps);
                }
            }

            if avg_delay < DELAY_THRESHOLD_150MS {
                user.delay.increase_fps_count += 1;
            } else {
                user.delay.increase_fps_count = 0;
            }
            if user.delay.increase_fps_count >= 3 {
                // After 3 stable samples, try increasing FPS
                user.delay.increase_fps_count = 0;
                fps += 1;
            }

            // Reset quick increase counter if network condition worsens
            if avg_delay > 50 {
                user.delay.quick_increase_fps_count = 0;
            }

            if bitrate_first {
                // While the bitrate can still come down, the frame rate keeps its floor.
                fps = fps.max(min_fps);
            }
            fps = fps.max(MIN_AUTO_FPS.min(user_cap));
            fps = user
                .delay
                .limit_fps_change(current_fps, fps, delay, bitrate_first, braked);
            fps = user
                .delay
                .accelerate_startup(current_fps, fps, user_cap, delay, braked);
            reduce_bitrate = bitrate_first
                && user.delay.needs_bitrate_reduction()
                && user.delay.replies_after_bitrate_reduction.is_none();
            fps = fps.clamp(MIN_FPS, user_cap);
            // first network delay message
            adjust_ratio = user.delay.fps.is_none();
            user.delay.fps = Some(fps);
            let base = user.delay.rtt_calculator.get_rtt().unwrap_or_default();
            log::trace!(
                "qos_trace t={} id={id} delay={delay} base={base} excess={} avg={avg_delay} bad={} good={} braked={braked} fps={fps} ratio={:.3} reduce_bitrate={reduce_bitrate}",
                hbb_common::get_time(),
                delay.saturating_sub(base),
                user.delay.consecutive_bad_samples,
                user.delay.good_samples,
                self.ratio,
            );
        }
        self.adjust_fps();
        // A viewer's first reply is one more trigger of the periodic adjustment and
        // keeps its cooldown: a viewer joining right after a cut must not spend the
        // other viewers' evidence a second time.
        if adjust_ratio
            && self.first_reply_adjusts_ratio
            && self.since(self.adjust_ratio_instant).as_secs() >= ADJUST_RATIO_INTERVAL as u64
        {
            self.adjust_ratio(false);
        }
        if reduce_bitrate && self.ratio_adjust_allowed() {
            self.adjust_ratio(false);
        }
    }

    pub fn user_delay_response_elapsed(&mut self, id: i32, elapsed: u128) {
        let Some(user) = self.users.get_mut(&id) else {
            return;
        };
        if elapsed <= 2000 {
            return;
        }
        user.delay.stall_ticks = user.delay.stall_ticks.saturating_add(1);
        user.delay.add_delay(elapsed as u32);
        user.delay.note_backlog();
        // Halve for every second the probe stays out beyond the first: two seconds
        // halve, three quarter, and so on down to the floor.
        let reference = match user.delay.stall_reference_fps {
            Some(reference) => reference,
            None => {
                let reference = user.delay.fps.unwrap_or(INIT_FPS.min(user.fps_cap()));
                user.delay.stall_reference_fps = Some(reference);
                user.delay.on_reduction(reference);
                reference
            }
        };
        let divisor = 1u32 << ((elapsed / 1000) as u32).saturating_sub(1).min(5);
        let user_cap = user.fps_cap();
        // The floor is a floor, not a lift: a target already below it stays.
        let current = user.delay.fps.unwrap_or(reference);
        let fps = (reference / divisor)
            .clamp(MIN_AUTO_FPS.min(user_cap), user_cap)
            .min(current);
        user.delay.fps = Some(fps);
        log::debug!(
            "qos_trace t={} id={id} timeout={elapsed} fps={fps}",
            hbb_common::get_time()
        );
        self.adjust_fps();
        // Replies are what usually drives the bitrate, and a deep queue is exactly when
        // they stop arriving: without this the controller would hold its bitrate for as
        // long as the backlog keeps the probes from coming back.
        if self.ratio_adjust_allowed() {
            self.adjust_ratio(false);
        }
    }
}
