use super::*;

pub(super) const TEST_DELAY_TIMEOUT: Duration = Duration::from_secs(1);
pub(super) const SEC30: Duration = Duration::from_secs(30);
pub(super) const H1: Duration = Duration::from_secs(3600);
pub(super) const MILLI1: Duration = Duration::from_millis(1);
pub(super) const SEND_TIMEOUT_VIDEO: u64 = 12_000;
pub(super) const SEND_TIMEOUT_OTHER: u64 = SEND_TIMEOUT_VIDEO * 10;
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
