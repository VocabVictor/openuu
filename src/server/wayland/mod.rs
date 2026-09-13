use super::*;
use hbb_common::{allow_err, anyhow};
use base::platform::linux::DISTRO;
use scrap::{
    is_cursor_embedded, set_map_err,
    wayland::pipewire::{fill_displays, try_fix_logical_size},
    Capturer, Display, Frame, TraitCapturer,
};
use std::collections::HashMap;
use std::io;

use crate::{
    client::{
        SCRAP_OTHER_VERSION_OR_X11_REQUIRED, SCRAP_UBUNTU_HIGHER_REQUIRED,
        SCRAP_X11_REQUIRED, SCRAP_XDP_PORTAL_UNAVAILABLE,
    },
    platform::linux::is_x11,
};

lazy_static::lazy_static! {
    static ref CAP_DISPLAY_INFO: RwLock<HashMap<usize, u64>> = RwLock::new(HashMap::new());
    static ref PIPEWIRE_INITIALIZED: RwLock<bool> = RwLock::new(false);
    static ref LOG_SCRAP_COUNT: Mutex<u32> = Mutex::new(0);
    static ref LAST_STAGE_ERR: Mutex<Option<(String, std::time::Instant)>> = Mutex::new(None);
    static ref ACTIVE_DISPLAY_COUNT: RwLock<usize> = RwLock::new(0);
}

mod counters_errors;
pub use counters_errors::*;
mod messages;
use messages::*;
#[cfg(test)]
mod tests;
mod capturer_types;
use capturer_types::*;
#[cfg(feature = "drm")]
mod uinput_rect;
#[cfg(feature = "drm")]
pub(super) use uinput_rect::*;
mod init;
pub(super) use init::*;
mod displays;
pub use displays::*;
mod capturer;
pub use capturer::*;
