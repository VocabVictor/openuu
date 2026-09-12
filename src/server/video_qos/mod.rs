use super::*;
use scrap::codec::{Quality, BR_BALANCED, BR_BEST, BR_SPEED};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/*
FPS adjust:
a. new user connected => set to INIT_FPS
b. TestDelay reply => update the user's fps from the excess delay, the reply's delay
   above the baseline this connection has shown so far:
     startup: two consecutive replies with excess < 50 ms permit doubling toward
       the viewer's cap; a higher excess or a brake ends this acceleration;
     excess < DELAY_THRESHOLD_150MS: a good reply; grows the fps, and after a
       reduction returns to the level held before it after two good replies;
     excess >= DELAY_THRESHOLD_150MS: a bad reply; nothing happens until three in a
       row confirm congestion, including after each reduction. FPS drops by a
       fifth at most; a second of excess cannot wait and halves it immediately.
       A recent fast restore also permits halving at 600 ms of excess.
   While the bitrate can still be reduced (ABR) it is reduced first and the fps keeps
   a floor: bitrate-targeted encoders do not send fewer bytes at fewer frames.
c. probe outstanding for more than two seconds => halve the fps for every further
   second, down to MIN_AUTO_FPS and never above the target it found; the late
   reply does not reduce again. Automatic reductions respect this floor unless
   the viewer requested a lower FPS cap.
d. second timeout / TestDelay reply => real fps is the minimum over all users;
   every user starts at INIT_FPS, adapts from its own target and is capped by its
   own limit, never by that minimum or by another user's limit

ratio adjust:
a. user set image quality => update to the maximum ratio of the latest quality
b. 3 seconds timeout => update ratio according to network delay
    When network delay < DELAY_THRESHOLD_150MS, increase ratio, max 150kbps;
    When a user calls for a reduction (two bad replies in a row, or a probe still
    out at the second tick past two seconds), decrease ratio by the step that user's
    own delay and confirmation call for, the most conservative step over all users;
    one slow reply or one short stall does not, and one user's spike is never paired
    with another user's confirmation.
c. confirmed congestion => decrease ratio at once, when the 3 seconds cooldown allows

delay:
    TestDelay shares the video stream, so it measures the queue in front of it rather
    than the path RTT. The baseline starts at the first reply and follows lower
    delays immediately. Old minima expire after 20 fresh replies; a higher window
    minimum is learned gradually only when the recent floor is no longer rising.
    Outstanding-probe checks and their late replies do not age this window.
*/

// Constants
pub const FPS: u32 = 30;
pub const MIN_FPS: u32 = 1;
pub const MAX_FPS: u32 = 120;
pub const INIT_FPS: u32 = 15;
const MIN_AUTO_FPS: u32 = 5;

// Bitrate ratio constants for different quality levels
const BR_MAX: f32 = 40.0; // 2000 * 2 / 100
const BR_MIN: f32 = 0.2;
const BR_MIN_HIGH_RESOLUTION: f32 = 0.1; // For high resolution, BR_MIN is still too high, so we set a lower limit
const MAX_BR_MULTIPLE: f32 = 1.0;

const HISTORY_DELAY_LEN: usize = 2;
const ADJUST_RATIO_INTERVAL: usize = 3; // Adjust quality ratio every 3 seconds
const DYNAMIC_SCREEN_THRESHOLD: usize = 2; // Allow increase quality ratio if encode more than 2 times in one second
const DELAY_THRESHOLD_150MS: u32 = 150; // 150ms is the threshold for good network condition
const RESTORE_GUARD_SAMPLES: u8 = 5; // A restored level that congests this soon is lowered

mod user_delay;
use user_delay::*;
mod qos_basic;
mod qos_sessions;
mod qos_adjust;
mod rtt;
use rtt::*;

// User session data structure
#[derive(Default, Debug, Clone)]
struct UserData {
    auto_adjust_fps: Option<u32>, // reserve for compatibility
    custom_fps: Option<u32>,
    quality: Option<(i64, Quality)>, // (time, quality)
    delay: UserDelay,
    record: bool,
    joined_at: Option<Instant>, // set by on_connection_open; the start-up guard's clock
}

impl UserData {
    // The frame rate this viewer asked for, from its custom or auto-adjust limit.
    fn fps_cap(&self) -> u32 {
        let mut fps = self.custom_fps.unwrap_or(FPS);
        if let Some(auto_adjust_fps) = self.auto_adjust_fps {
            if fps == 0 || auto_adjust_fps < fps {
                fps = auto_adjust_fps;
            }
        }
        fps.clamp(MIN_FPS, MAX_FPS)
    }
}

#[derive(Default, Debug, Clone)]
struct DisplayData {
    send_counter: usize, // Number of times encode during period
    support_changing_quality: bool,
}

// Main QoS controller structure
pub struct VideoQoS {
    fps: u32,
    ratio: f32,
    users: HashMap<i32, UserData>,
    displays: HashMap<String, DisplayData>,
    bitrate_store: u32,
    adjust_ratio_instant: Instant,
    abr_config: bool,
    first_reply_adjusts_ratio: bool, // false on Linux, where it can create vaapi twice
    #[cfg(test)]
    test_now: Option<Instant>,
}

impl Default for VideoQoS {
    fn default() -> Self {
        VideoQoS {
            fps: FPS,
            ratio: BR_BALANCED,
            users: Default::default(),
            displays: Default::default(),
            bitrate_store: 0,
            adjust_ratio_instant: Instant::now(),
            abr_config: true,
            first_reply_adjusts_ratio: !cfg!(target_os = "linux"),
            #[cfg(test)]
            test_now: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stable_qos() -> VideoQoS {
        let mut qos = VideoQoS::default();
        qos.advance_ms(2000);
        qos.users.insert(1, UserData::default());
        for _ in 0..12 {
            qos.user_network_delay(1, 10);
        }
        assert_eq!(qos.fps(), FPS);
        qos
    }

    #[test]
    fn isolated_delay_spike_does_not_lower_fps() {
        let mut qos = stable_qos();
        for delay in [800, 10, 10, 10] {
            qos.user_network_delay(1, delay);
            assert_eq!(qos.fps(), FPS);
        }
    }

    #[test]
    fn occasional_spikes_do_not_accumulate_congestion() {
        let mut qos = stable_qos();
        for delay in [800, 10, 10].repeat(20) {
            qos.user_network_delay(1, delay);
            assert_eq!(qos.fps(), FPS);
        }
    }

    #[test]
    fn sustained_delay_reduces_fps_gradually() {
        let mut qos = stable_qos();
        for expected_fps in [30, 30, 24, 24, 24, 20] {
            qos.user_network_delay(1, 800);
            assert_eq!(qos.fps(), expected_fps);
        }
    }

    #[test]
    fn delay_history_keeps_two_samples() {
        let mut delay = UserDelay::default();
        for sample in [1, 2, 3] {
            delay.add_delay(sample);
        }
        assert_eq!(delay.delay_history.len(), HISTORY_DELAY_LEN);
    }

    #[test]
    fn response_timeout_halves_fps_for_each_second_outstanding() {
        let mut qos = stable_qos();
        for (elapsed, expected) in [(2001, 15), (3001, 7), (4001, 5), (5001, 5), (6001, 5)] {
            qos.user_delay_response_elapsed(1, elapsed);
            assert_eq!(qos.fps(), expected, "{elapsed} ms outstanding");
        }
    }

    #[test]
    fn severe_delay_does_not_wait_for_another_reply() {
        let mut qos = stable_qos();
        qos.user_network_delay(1, 1200);
        assert_eq!(qos.fps(), 15);
    }

    #[test]
    fn response_timeout_recovers_in_two_good_replies() {
        let mut qos = stable_qos();
        qos.user_delay_response_elapsed(1, 3000);
        assert_eq!(qos.fps(), 7);
        qos.user_network_delay(1, 3200);
        assert_eq!(
            qos.fps(),
            7,
            "the late reply belongs to the stall that was braked"
        );
        qos.user_delay_response_elapsed(1, 0);
        qos.user_network_delay(1, 10);
        assert_eq!(
            qos.fps(),
            8,
            "one good reply must not restore the full frame rate"
        );
        qos.user_network_delay(1, 10);
        assert_eq!(
            qos.fps(),
            FPS,
            "the second good reply restores the frame rate"
        );
        qos.user_network_delay(1, 10);
        assert_eq!(qos.fps(), FPS, "the third keeps the restored frame rate");
    }

    #[test]
    fn restore_aims_lower_after_a_restore_that_congested() {
        let mut qos = stable_qos();
        for _ in 0..2 {
            qos.user_network_delay(1, 1200);
        }
        assert_eq!(qos.fps(), 8);
        qos.user_network_delay(1, 10);
        qos.user_network_delay(1, 10);
        assert_eq!(qos.fps(), FPS);
        // The restored level congests at once, so the next restore aims lower.
        for _ in 0..3 {
            qos.user_network_delay(1, 400);
        }
        assert!(qos.fps() < FPS);
        qos.user_network_delay(1, 10);
        qos.user_network_delay(1, 10);
        assert!(
            qos.fps() < FPS,
            "no return to the level that failed: {}",
            qos.fps()
        );
        qos.user_network_delay(1, 10);
        assert_eq!(qos.fps(), FPS, "ordinary recovery can still reach the cap");
    }

    #[test]
    fn custom_fps_limit_applies_during_delay_spike() {
        let mut qos = stable_qos();
        qos.user_custom_fps(1, 12);
        qos.user_network_delay(1, 800);
        assert_eq!(qos.fps(), 12);
    }

    mod adaptation;
    mod baseline;
    mod invariants;
    mod jitter;
    mod recovery;
    mod robustness;
    mod sim;
    mod smoke;
    mod startup;
}
