use super::*;

/// Check if exit shortcut for relative mouse mode is active.
/// Exit shortcuts (only exits, not toggles):
/// - macOS: Cmd+G
/// - Windows/Linux: Ctrl+Alt (triggered when both are pressed)
/// Note: This shortcut is only available in Flutter client. Sciter client does not support relative mouse mode.
#[cfg(feature = "flutter")]
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn is_exit_relative_mouse_shortcut(key: Key) -> bool {
    let modifiers = MODIFIERS_STATE.lock().unwrap();

    #[cfg(target_os = "macos")]
    {
        // macOS: Cmd+G to exit
        if key != Key::KeyG {
            return false;
        }
        let meta = *modifiers.get(&Key::MetaLeft).unwrap_or(&false)
            || *modifiers.get(&Key::MetaRight).unwrap_or(&false);
        return meta;
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows/Linux: Ctrl+Alt to exit
        // Triggered when Ctrl is pressed while Alt is down, or Alt is pressed while Ctrl is down
        let is_ctrl_key = key == Key::ControlLeft || key == Key::ControlRight;
        let is_alt_key = key == Key::Alt || key == Key::AltGr;

        if !is_ctrl_key && !is_alt_key {
            return false;
        }

        let ctrl = *modifiers.get(&Key::ControlLeft).unwrap_or(&false)
            || *modifiers.get(&Key::ControlRight).unwrap_or(&false);
        let alt = *modifiers.get(&Key::Alt).unwrap_or(&false)
            || *modifiers.get(&Key::AltGr).unwrap_or(&false);

        // When Ctrl is pressed and Alt is already down, or vice versa
        (is_ctrl_key && alt) || (is_alt_key && ctrl)
    }
}

/// Notify Flutter to exit relative mouse mode.
/// Note: This is Flutter-only. Sciter client does not support relative mouse mode.
#[cfg(feature = "flutter")]
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn notify_exit_relative_mouse_mode() {
    let session_id = flutter::get_cur_session_id();
    flutter::push_session_event(&session_id, "exit_relative_mouse_mode", vec![]);
}

/// Handle relative mouse mode shortcuts in the rdev grab loop.
/// Returns true if the event should be blocked from being sent to the peer.
#[cfg(feature = "flutter")]
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[inline]
fn can_exit_relative_mouse_mode_from_grab_loop() -> bool {
    // Only process exit shortcuts when relative mouse mode is actually active.
    // This prevents blocking Ctrl+Alt (or Cmd+G) when not in relative mouse mode.
    if !RELATIVE_MOUSE_MODE_ACTIVE.load(Ordering::SeqCst) {
        return false;
    }

    let Some(session) = flutter::get_cur_session() else {
        return false;
    };

    // Only for remote desktop sessions.
    if !session.is_default() {
        return false;
    }

    // Must have keyboard permission and not be in view-only mode.
    if !*session.server_keyboard_enabled.read().unwrap() {
        return false;
    }
    let lc = session.lc.read().unwrap();
    if lc.get_toggle_option("view-only") {
        return false;
    }

    // Peer must support relative mouse mode.
    crate::common::is_support_relative_mouse_mode_num(lc.version)
}

#[cfg(feature = "flutter")]
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[inline]
fn should_block_relative_mouse_shortcut(key: Key, is_press: bool) -> bool {
    if !KEYBOARD_HOOKED.load(Ordering::SeqCst) {
        return false;
    }

    // Determine which key to track for key-up blocking based on platform
    #[cfg(target_os = "macos")]
    let is_tracked_key = key == Key::KeyG;
    #[cfg(not(target_os = "macos"))]
    let is_tracked_key = key == Key::ControlLeft
        || key == Key::ControlRight
        || key == Key::Alt
        || key == Key::AltGr;

    // Block key up if key down was blocked (to avoid orphan key up event on remote).
    // This must be checked before clearing the flag below.
    if is_tracked_key && !is_press && EXIT_SHORTCUT_KEY_DOWN.swap(false, Ordering::SeqCst) {
        return true;
    }

    // Exit relative mouse mode shortcuts:
    // - macOS: Cmd+G
    // - Windows/Linux: Ctrl+Alt
    // Guard it to supported/eligible sessions to avoid blocking the chord unexpectedly.
    if is_exit_relative_mouse_shortcut(key) {
        if !can_exit_relative_mouse_mode_from_grab_loop() {
            return false;
        }
        if is_press {
            // Only trigger exit on transition from "not pressed" to "pressed".
            // This prevents retriggering on OS key-repeat.
            if !EXIT_SHORTCUT_KEY_DOWN.swap(true, Ordering::SeqCst) {
                notify_exit_relative_mouse_mode();
            }
        }
        return true;
    }

    false
}

pub(super) fn start_grab_loop() {
    std::env::set_var("KEYBOARD_ONLY", "y");
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    std::thread::spawn(move || {
        let try_handle_keyboard = move |event: Event, key: Key, is_press: bool| -> Option<Event> {
            // fix #2211：CAPS LOCK don't work
            if key == Key::CapsLock || key == Key::NumLock {
                return Some(event);
            }

            let _scan_code = event.position_code;
            let _code = event.platform_code as KeyCode;

            #[cfg(feature = "flutter")]
            if should_block_relative_mouse_shortcut(key, is_press) {
                return None;
            }

            let res = if KEYBOARD_HOOKED.load(Ordering::SeqCst) {
                client::process_event(&get_keyboard_mode(), &event, None);
                if is_press {
                    None
                } else {
                    Some(event)
                }
            } else {
                Some(event)
            };

            #[cfg(target_os = "windows")]
            match _scan_code {
                0x1D | 0x021D => rdev::set_modifier(Key::ControlLeft, is_press),
                0xE01D => rdev::set_modifier(Key::ControlRight, is_press),
                0x2A => rdev::set_modifier(Key::ShiftLeft, is_press),
                0x36 => rdev::set_modifier(Key::ShiftRight, is_press),
                0x38 => rdev::set_modifier(Key::Alt, is_press),
                // Right Alt
                0xE038 => rdev::set_modifier(Key::AltGr, is_press),
                0xE05B => rdev::set_modifier(Key::MetaLeft, is_press),
                0xE05C => rdev::set_modifier(Key::MetaRight, is_press),
                _ => {}
            }

            #[cfg(target_os = "windows")]
            unsafe {
                // AltGr
                if _scan_code == 0x021D {
                    IS_0X021D_DOWN = is_press;
                }
            }

            #[cfg(target_os = "macos")]
            unsafe {
                if _code == rdev::kVK_Option {
                    IS_LEFT_OPTION_DOWN = is_press;
                }
            }

            return res;
        };
        let func = move |event: Event| match event.event_type {
            EventType::KeyPress(key) => try_handle_keyboard(event, key, true),
            EventType::KeyRelease(key) => try_handle_keyboard(event, key, false),
            _ => Some(event),
        };
        #[cfg(target_os = "macos")]
        rdev::set_is_main_thread(false);
        #[cfg(target_os = "windows")]
        rdev::set_event_popup(false);
        if let Err(error) = rdev::grab(func) {
            log::error!("rdev Error: {:?}", error)
        }
    });

    #[cfg(target_os = "linux")]
    if let Err(err) = rdev::start_grab_listen(move |event: Event| match event.event_type {
        EventType::KeyPress(key) | EventType::KeyRelease(key) => {
            let is_press = matches!(event.event_type, EventType::KeyPress(_));
            if let Key::Unknown(keycode) = key {
                log::error!("rdev get unknown key, keycode is {:?}", keycode);
            } else {
                #[cfg(feature = "flutter")]
                if should_block_relative_mouse_shortcut(key, is_press) {
                    return None;
                }
                client::process_event(&get_keyboard_mode(), &event, None);
            }
            None
        }
        _ => Some(event),
    }) {
        log::error!("Failed to init rdev grab thread: {:?}", err);
    };
}

// #[allow(dead_code)] is ok here. No need to stop grabbing loop.
#[allow(dead_code)]
fn stop_grab_loop() -> Result<(), rdev::GrabError> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    rdev::exit_grab()?;
    #[cfg(target_os = "linux")]
    rdev::exit_grab_listen();
    Ok(())
}
