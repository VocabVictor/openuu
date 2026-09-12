#[cfg(target_os = "linux")]
use super::rdp_input::client::{RdpInputKeyboard, RdpInputMouse};
use super::*;
use crate::input::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::whiteboard;
use base::message_proto::{
    pointer_device_event::Union::TouchEvent, touch_event::Union::ScaleUpdate,
};
#[cfg(target_os = "macos")]
use dispatch::Queue;
use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::{get_time, protobuf::EnumOrUnknown};
use rdev::{self, EventType, Key as RdevKey, KeyCode, RawKey};
#[cfg(target_os = "macos")]
use rdev::{CGEventSourceStateID, CGEventTapLocation, VirtualInput};
#[cfg(target_os = "linux")]
use scrap::wayland::pipewire::RDP_SESSION_INFO;
#[cfg(target_os = "linux")]
use std::sync::mpsc;
use std::{
    convert::TryFrom,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{self, Instant},
};

#[cfg(windows)]
use winapi::um::winuser::WHEEL_DELTA;

const INVALID_CURSOR_POS: i32 = i32::MIN;
const INVALID_DISPLAY_IDX: i32 = -1;

#[derive(Default, Clone, Copy)]
struct Input {
    conn: i32,
    time: i64,
    x: i32,
    y: i32,
}

const KEY_CHAR_START: u64 = 9999;

// XKB keycode for Insert key (evdev KEY_INSERT code 110 + 8 for XKB offset)
#[cfg(target_os = "linux")]
const XKB_KEY_INSERT: u16 = evdev::Key::KEY_INSERT.code() + 8;

mod services;
pub use services::*;
mod lock_modes;
use lock_modes::*;
mod mouse_state;
pub use mouse_state::*;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform_input;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use platform_input::*;
mod mouse;
pub use mouse::*;
mod mouse_simulation;
pub use mouse_simulation::*;
mod key_state;
pub use key_state::*;
mod key_handle;
pub use key_handle::*;
mod key_modifiers;
use key_modifiers::*;
#[cfg(target_os = "linux")]
mod wayland_clipboard;
#[cfg(target_os = "linux")]
use wayland_clipboard::*;
mod keyboard_modes;
use keyboard_modes::*;
#[cfg(target_os = "linux")]
mod wayland_input;
#[cfg(target_os = "linux")]
pub use wayland_input::*;
mod key_maps;
use key_maps::*;
