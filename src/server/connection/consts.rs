use super::*;

pub(super) const TEST_DELAY_TIMEOUT: Duration = Duration::from_secs(1);
pub(super) const SEC30: Duration = Duration::from_secs(30);
pub(super) const H1: Duration = Duration::from_secs(3600);
pub(super) const MILLI1: Duration = Duration::from_millis(1);
/// How long one write to a controlling peer may take before the session is given up on.
///
/// Twelve seconds was long past the point where anyone is still waiting: a session that
/// has not moved a byte for that long has already been abandoned by the person using it.
/// It cannot go as low as the couple of seconds that would be ideal, because the longest
/// single write is a key frame, and on a link that has just collapsed the key frame in
/// flight was sized for the link as it was. Five seconds is longer than the controller
/// needs to measure the new link and cut the bitrate to fit it, which is what
/// `a_write_may_take_longer_than_the_controller_needs_to_react` holds it to.
pub(super) const SEND_TIMEOUT_VIDEO: u64 = 5_000;
/// File transfer, terminal and port forward: a single write can legitimately be a large
/// block on a slow link, and nobody is watching a picture while it happens.
pub(super) const SEND_TIMEOUT_OTHER: u64 = 120_000;
pub(super) const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether the DRM backend can serve a Wayland login screen here.
///
/// A cold cache probes off-thread; admission still requires a definitive `Available` verdict.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(super) fn drm_can_serve_login_screen() -> bool {
    super::super::drm_capturer::availability_cached() == super::super::drm_capturer::Availability::Available
}

/// Without the feature nothing can capture a Wayland greeter, so the refusal stands.
#[cfg(all(target_os = "linux", not(feature = "drm")))]
pub(super) fn drm_can_serve_login_screen() -> bool {
    false
}
