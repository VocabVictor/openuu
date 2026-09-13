//! How long a write may take before the session is given up on.
use super::*;

/// The write timeout has to outlast the controller's reaction to a link that collapsed,
/// or a session that would have recovered is killed instead. The controller needs a
/// second of blocked sends to measure the link, and then one adjustment window to cut
/// the bitrate to fit it.
#[test]
fn a_write_may_take_longer_than_the_controller_needs_to_react() {
    let measure = video_qos::BLOCKED_MS_FOR_CAPACITY as u64;
    let react = measure + 3_000;
    assert!(
        SEND_TIMEOUT_VIDEO > react,
        "{SEND_TIMEOUT_VIDEO} ms leaves no room for the controller's {react} ms"
    );
}

/// And it has to be short enough that a session nobody can use is not held open. Twelve
/// seconds, what it used to be, is longer than anyone waits before reconnecting.
#[test]
fn a_dead_session_is_not_held_open_for_longer_than_anyone_waits() {
    assert!(SEND_TIMEOUT_VIDEO <= 6_000, "{SEND_TIMEOUT_VIDEO} ms");
}

/// File transfer is the other case: a single block on a slow link is not a dead session,
/// and no one is watching a picture while it goes.
#[test]
fn a_file_transfer_write_is_given_much_longer() {
    assert!(SEND_TIMEOUT_OTHER >= 20 * SEND_TIMEOUT_VIDEO);
}
