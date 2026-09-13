// Unprivileged consumer of the root `--service`'s DRM/KMS capture stream: the service does the
// privileged export (open + grab the scanout dma-buf fd), the EGL detile / RGBA convert runs here.

use crate::ipc::{connect_drm, Data, DrmDisplayInfo};
use hbb_common::{anyhow::anyhow, bail, log, tokio, ResultType};
use base::message_proto::DisplayInfo;
use scrap::drm_render::RenderConverter;
use scrap::drmtap_dl::drmtap_dmabuf_desc;
use scrap::{Frame, Pixfmt, PixelBuffer, TraitCapturer};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const HANDSHAKE_TIMEOUT_MS: u64 = 3000;
const DRM_CONNECT_TIMEOUT_MS: u64 = 1000;
/// The service may hold the list back while it wakes sleeping displays: ~3.6s (DRM_WAKE_*).
const DISPLAY_LIST_TIMEOUT_MS: u64 = HANDSHAKE_TIMEOUT_MS + 4000;
/// Covers the connect timeout plus `recv_msg_timeout2` applying DISPLAY_LIST_TIMEOUT_MS TWICE
/// (first byte, then body). The render-node open and the DrmStart send can still overrun it.
const HANDSHAKE_WAIT_MS: u64 = DRM_CONNECT_TIMEOUT_MS + DISPLAY_LIST_TIMEOUT_MS * 2 + 500;
/// Only the header read rechecks `stop`, so bound the body read here rather than relying on
    /// `next_raw_into`'s own cap.
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(5);

mod types;
pub use types::*;
mod geometry;
use geometry::*;
mod health;
pub(super) use health::*;
mod capturer_impl;
mod capturer_frame;
mod recv;
use recv::*;
mod cursor;
pub use cursor::*;
mod probe;
use probe::*;
mod availability;
pub(crate) use availability::*;
mod display_infos;
pub(super) use display_infos::*;
mod wayland_geometry;
use wayland_geometry::*;
mod capturer_info;
pub(super) use capturer_info::*;

/// A delivered frame resets the streak verdicts (`zero_frame_streak`, `demotes`, `since`) and
    /// nothing else.

#[cfg(test)]
mod drm_capturer_tests {
    use super::*;

    mod tests_a;
    use tests_a::*;
    mod tests_b;
    mod tests_c;

}
