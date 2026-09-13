//! When an empty capture means the desktop duplication is not going to work here.
//!
//! An empty capture is how DXGI says "nothing changed", and on a desktop nobody is
//! touching that is every capture there will ever be. It is also how a duplication that
//! will never deliver anything behaves, and the only difference between the two is how
//! long it goes on for. The old rule was four empty captures in a row, which at 30 fps is
//! a tenth of a second of a still screen, and the fallback it leads to costs a full frame
//! compare and copy on every capture from then until the session is switched.

use std::time::Duration;

/// Empty captures in a row before the duplication is given up on. Time is the real
/// criterion; this only keeps a single slow first capture from counting as a run.
const EMPTY_CAPTURES: u32 = 4;
/// How long a session may go without its first image before GDI is used instead. A
/// duplication that works delivers the desktop it was created for well inside this;
/// a still screen that has nothing to send looks the same but costs nothing to wait for.
const WITHOUT_AN_IMAGE: Duration = Duration::from_secs(2);

/// Whether a session that has never captured an image should fall back to GDI.
pub(super) fn give_up_on_duplication(empty_captures: u32, since_start: Duration) -> bool {
    empty_captures >= EMPTY_CAPTURES && since_start >= WITHOUT_AN_IMAGE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_screen_for_a_moment_is_not_a_broken_duplication() {
        // Four empty captures at 30 fps take about a tenth of a second, which is what the
        // old rule gave up after.
        assert!(!give_up_on_duplication(4, Duration::from_millis(132)));
        assert!(!give_up_on_duplication(60, Duration::from_millis(1_999)));
    }

    #[test]
    fn a_session_that_never_gets_an_image_falls_back() {
        assert!(give_up_on_duplication(4, WITHOUT_AN_IMAGE));
        assert!(give_up_on_duplication(100, Duration::from_secs(10)));
    }

    #[test]
    fn one_slow_capture_is_not_a_run_however_long_it_took() {
        assert!(!give_up_on_duplication(1, Duration::from_secs(30)));
        assert!(!give_up_on_duplication(3, Duration::from_secs(30)));
    }

    /// The rate the loop runs at must not change the rule: five frames a second reaches
    /// four empty captures in 800 ms, thirty in 132 ms, and neither is evidence.
    #[test]
    fn the_frame_rate_does_not_decide_it() {
        for fps in [5u32, 15, 30, 60] {
            let per_capture = Duration::from_millis(1_000 / fps as u64);
            let mut empty = 0;
            let mut elapsed = Duration::ZERO;
            while !give_up_on_duplication(empty, elapsed) {
                empty += 1;
                elapsed += per_capture;
                assert!(elapsed < Duration::from_secs(3), "fps {fps} never gave up");
            }
            assert!(elapsed >= WITHOUT_AN_IMAGE, "fps {fps} gave up after {elapsed:?}");
        }
    }
}
