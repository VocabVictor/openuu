#[cfg(feature = "flutter")]
use crate::flutter;
#[cfg(target_os = "windows")]
use crate::platform::windows::{get_char_from_vk, get_unicode_from_vk};
use crate::ui_session_interface::{InvokeUiSession, Session};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::{client::get_key_state, common::GrabState};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::log;
use base::message_proto::*;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use rdev::KeyCode;
use rdev::{Event, EventType, Key};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub mod client;
mod grab_loop;
mod keys;
pub use keys::*;
mod legacy;
pub use legacy::*;
mod map_mode;
pub use map_mode::*;
mod translate_mode;
pub use translate_mode::*;
#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod input_source;
use grab_loop::start_grab_loop;
use keys::{is_letter_key_4_lock_modes, is_numpad_key, parse_add_lock_modes_modifiers, release_remote_keys_for_events, take_remote_keys, update_modifiers_state};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use keys::add_lock_modes_modifiers;
use map_mode::windows_peer_special_key;

#[cfg(windows)]
static mut IS_ALT_GR: bool = false;

#[allow(dead_code)]
const OS_LOWER_WINDOWS: &str = "windows";
#[allow(dead_code)]
const OS_LOWER_LINUX: &str = "linux";
#[allow(dead_code)]
const OS_LOWER_MACOS: &str = "macos";
#[allow(dead_code)]
const OS_LOWER_ANDROID: &str = "android";

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);

// Track key down state for relative mouse mode exit shortcut.
// macOS: Cmd+G (track G key)
// Windows/Linux: Ctrl+Alt (track whichever modifier was pressed last)
// This prevents the exit from retriggering on OS key-repeat.
#[cfg(all(feature = "flutter", any(target_os = "windows", target_os = "macos", target_os = "linux")))]
static EXIT_SHORTCUT_KEY_DOWN: AtomicBool = AtomicBool::new(false);

// Track whether relative mouse mode is currently active.
// This is set by Flutter via set_relative_mouse_mode_state() and checked
// by the rdev grab loop to determine if exit shortcuts should be processed.
#[cfg(all(feature = "flutter", any(target_os = "windows", target_os = "macos", target_os = "linux")))]
static RELATIVE_MOUSE_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Set the relative mouse mode state from Flutter.
/// This is called when entering or exiting relative mouse mode.
#[cfg(all(feature = "flutter", any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn set_relative_mouse_mode_state(active: bool) {
    RELATIVE_MOUSE_MODE_ACTIVE.store(active, Ordering::SeqCst);
    // Reset exit shortcut state when mode changes to avoid stale state
    if !active {
        EXIT_SHORTCUT_KEY_DOWN.store(false, Ordering::SeqCst);
    }
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
static IS_RDEV_ENABLED: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref TO_RELEASE: Arc<Mutex<HashMap<Key, Event>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref MODIFIERS_STATE: Mutex<HashMap<Key, bool>> = {
        let mut m = HashMap::new();
        m.insert(Key::ShiftLeft, false);
        m.insert(Key::ShiftRight, false);
        m.insert(Key::ControlLeft, false);
        m.insert(Key::ControlRight, false);
        m.insert(Key::Alt, false);
        m.insert(Key::AltGr, false);
        m.insert(Key::MetaLeft, false);
        m.insert(Key::MetaRight, false);
        Mutex::new(m)
    };
}

#[cfg(windows)]
pub fn update_grab_get_key_name(keyboard_mode: &str) {
    match keyboard_mode {
        "map" => rdev::set_get_key_unicode(false),
        "translate" => rdev::set_get_key_unicode(true),
        "legacy" => rdev::set_get_key_unicode(true),
        _ => {}
    };
}

#[cfg(target_os = "windows")]
static mut IS_0X021D_DOWN: bool = false;

#[cfg(target_os = "macos")]
static mut IS_LEFT_OPTION_DOWN: bool = false;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn get_keyboard_mode() -> String {
    #[cfg(feature = "flutter")]
    if let Some(session) = flutter::get_cur_session() {
        return session.get_keyboard_mode();
    }
    "legacy".to_string()
}

pub fn event_to_key_events(
    mut peer: String,
    event: &Event,
    keyboard_mode: KeyboardMode,
    _lock_modes: Option<i32>,
) -> Vec<KeyEvent> {
    peer.retain(|c| !c.is_whitespace());

    update_modifiers_state(event);

    match event.event_type {
        EventType::KeyPress(key) => {
            TO_RELEASE.lock().unwrap().insert(key, event.clone());
        }
        EventType::KeyRelease(key) => {
            TO_RELEASE.lock().unwrap().remove(&key);
        }
        _ => {}
    }

    let mut key_event = KeyEvent::new();
    key_event.mode = keyboard_mode.into();

    let mut key_events = match keyboard_mode {
        KeyboardMode::Map => map_keyboard_mode(peer.as_str(), event, key_event),
        KeyboardMode::Translate => translate_keyboard_mode(peer.as_str(), event, key_event),
        _ => {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                legacy_keyboard_mode(event, key_event)
            }
            #[cfg(any(target_os = "android", target_os = "ios"))]
            {
                Vec::new()
            }
        }
    };

    let is_numpad_key = is_numpad_key(&event);
    if keyboard_mode != KeyboardMode::Translate || is_numpad_key {
        let is_letter_key = is_letter_key_4_lock_modes(&event);
        for key_event in &mut key_events {
            if let Some(lock_modes) = _lock_modes {
                parse_add_lock_modes_modifiers(key_event, lock_modes, is_numpad_key, is_letter_key);
            } else {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                add_lock_modes_modifiers(key_event, is_numpad_key, is_letter_key);
            }
        }
    }
    key_events
}

pub fn send_key_event(key_event: &KeyEvent) {

    #[cfg(feature = "flutter")]
    if let Some(session) = flutter::get_cur_session() {
        session.send_key_event(key_event);
    }
}

pub fn get_peer_platform() -> String {
    #[cfg(feature = "flutter")]
    if let Some(session) = flutter::get_cur_session() {
        return session.peer_platform();
    }
    "Windows".to_string()
}


