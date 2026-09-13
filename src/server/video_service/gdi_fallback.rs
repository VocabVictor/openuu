//! When an empty capture means the desktop duplication is not going to work here.
//!
//! An empty capture is how DXGI says "nothing changed", and on a desktop nobody is
//! touching that is every capture there will ever be. It is also how a duplication that
//! will never deliver anything behaves, and the only difference between the two is how
//! long it goes on for. The old rule was four empty captures in a row, which at 30 fps is
//! a tenth of a second of a still screen, and the fallback it leads to costs a full frame
//! compare and copy on every capture from then until the session is switched.

use std::{io, time::Duration};

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


/// What a capture error leaves the loop to do.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AfterFailure {
    /// The duplication is gone rather than unavailable, so it is made again. The desktop
    /// it was made for has been replaced: a secure desktop for a prompt, a lock screen,
    /// a mode change.
    Rebuild,
    /// Desktop duplication does not work on this machine, or has stopped working often
    /// enough that waiting for it is worse than the slow path.
    UseGdi,
}

/// Consecutive rebuilds allowed before the duplication is given up on. Reset by a capture
/// that works, so this counts an episode, not a session.
const MAX_REBUILDS: u32 = 3;

/// What to do about a capture error. The kinds are what `libs/scrap`'s dxgi module maps
/// the DXGI codes to.
pub(super) fn after_capture_error(kind: io::ErrorKind, rebuilds: u32) -> AfterFailure {
    use io::ErrorKind::*;
    if rebuilds >= MAX_REBUILDS {
        return AfterFailure::UseGdi;
    }
    match kind {
        // DXGI_ERROR_ACCESS_LOST: the desktop was replaced, which on Windows happens
        // whenever a prompt puts the secure desktop up. Documented as "make the
        // duplication again", and answering it by giving up on the duplication is how a
        // single consent prompt used to cost a session its capture path.
        ConnectionReset => AfterFailure::Rebuild,
        // DXGI_ERROR_INVALID_CALL: a frame the capturer holds, or a desktop image it can
        // no longer map. Both are recovered by starting again.
        InvalidData => AfterFailure::Rebuild,
        // E_ACCESSDENIED: the desktop of the moment is not ours to read. GDI cannot read
        // it either, so the way out is the next desktop, not the other capture path.
        PermissionDenied => AfterFailure::Rebuild,
        // DXGI_ERROR_UNSUPPORTED, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE: this machine does
        // not offer duplication, and making it again would fail the same way.
        ConnectionRefused | Interrupted => AfterFailure::UseGdi,
        // Anything else is unknown, and an unknown error that a rebuild cannot fix would
        // cost a rebuild every frame. The log line at the call site is the record.
        _ => AfterFailure::UseGdi,
    }
}

/// How long to wait before making the duplication again. A desktop switch takes a moment
/// to settle, and a rebuild that fails immediately must not become a rebuild every frame.
pub(super) fn rebuild_backoff(rebuilds: u32) -> Duration {
    Duration::from_millis(100 << rebuilds.min(3))
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

    #[test]
    fn a_lost_desktop_is_made_again_rather_than_given_up_on() {
        // A consent prompt puts up a secure desktop, which the duplication does not
        // survive; the old rule spent the rest of the session on GDI for it.
        assert_eq!(
            after_capture_error(io::ErrorKind::ConnectionReset, 0),
            AfterFailure::Rebuild
        );
        assert_eq!(
            after_capture_error(io::ErrorKind::InvalidData, 0),
            AfterFailure::Rebuild
        );
        assert_eq!(
            after_capture_error(io::ErrorKind::PermissionDenied, 0),
            AfterFailure::Rebuild
        );
    }

    #[test]
    fn a_machine_without_duplication_goes_straight_to_gdi() {
        assert_eq!(
            after_capture_error(io::ErrorKind::ConnectionRefused, 0),
            AfterFailure::UseGdi
        );
        assert_eq!(
            after_capture_error(io::ErrorKind::Interrupted, 0),
            AfterFailure::UseGdi
        );
    }

    #[test]
    fn an_unknown_error_is_not_rebuilt_for() {
        for kind in [
            io::ErrorKind::Other,
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::TimedOut,
        ] {
            assert_eq!(after_capture_error(kind, 0), AfterFailure::UseGdi, "{kind:?}");
        }
    }

    #[test]
    fn rebuilding_gives_up_after_an_episode_of_it() {
        let kind = io::ErrorKind::ConnectionReset;
        for rebuilds in 0..MAX_REBUILDS {
            assert_eq!(after_capture_error(kind, rebuilds), AfterFailure::Rebuild);
        }
        assert_eq!(after_capture_error(kind, MAX_REBUILDS), AfterFailure::UseGdi);
        assert_eq!(after_capture_error(kind, 50), AfterFailure::UseGdi);
    }

    #[test]
    fn the_wait_grows_and_stops_growing() {
        let waits: Vec<Duration> = (0..6).map(rebuild_backoff).collect();
        assert!(waits.windows(2).all(|w| w[1] >= w[0]), "{waits:?}");
        assert_eq!(waits[0], Duration::from_millis(100));
        assert!(waits.iter().all(|wait| *wait <= Duration::from_secs(1)));
    }
}
