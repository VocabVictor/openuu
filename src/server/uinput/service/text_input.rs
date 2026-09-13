use super::*;

/// Input text on Wayland using layout-independent methods.
/// ASCII chars (0x20-0x7E): Portal keysym or uinput fallback
/// Non-ASCII chars: skipped — this runs in the --service (root) process where clipboard
/// operations are unreliable (typically no user session environment).
/// Non-ASCII input is normally handled by the --server process via input_text_via_clipboard_server.
pub(super) fn input_text_wayland(text: &str, keyboard: &mut VirtualDevice) {
    let portal_info = {
        let session_info = RDP_SESSION_INFO.lock().unwrap();
        session_info
            .as_ref()
            .map(|info| (info.conn.clone(), info.session.clone()))
    };

    for c in text.chars() {
        let keysym = char_to_keysym(c);
        if can_input_via_keysym(c, keysym) {
            // Try Portal first — down+up on the same channel
            if let Some((ref conn, ref session)) = portal_info {
                let portal = scrap::wayland::pipewire::get_portal(conn);
                if portal
                    .notify_keyboard_keysym(session, HashMap::new(), keysym, 1)
                    .is_ok()
                {
                    if let Err(e) =
                        portal.notify_keyboard_keysym(session, HashMap::new(), keysym, 0)
                    {
                        log::warn!(
                            "input_text_wayland: portal key-up failed for keysym {:#x}: {:?}",
                            keysym,
                            e
                        );
                    }
                    continue;
                }
            }
            // Portal unavailable or failed, fallback to uinput (down+up together)
            let key = enigo::Key::Layout(c);
            if let Ok((evdev_key, is_shift)) = map_key(&key) {
                let mut shift_pressed = false;
                if is_shift {
                    let shift_down =
                        InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 1);
                    if keyboard.emit(&[shift_down]).is_ok() {
                        shift_pressed = true;
                    } else {
                        log::warn!("input_text_wayland: failed to press Shift for '{}'", c);
                    }
                }
                let key_down = InputEvent::new(EventType::KEY, evdev_key.code(), 1);
                let key_up = InputEvent::new(EventType::KEY, evdev_key.code(), 0);
                allow_err!(keyboard.emit(&[key_down, key_up]));
                if shift_pressed {
                    let shift_up =
                        InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 0);
                    allow_err!(keyboard.emit(&[shift_up]));
                }
            }
        } else {
            log::debug!("Skipping non-ASCII character in uinput service (no clipboard access)");
        }
    }
}

/// Send a single key down or up event for a Layout character.
/// Used by KeyDown/KeyUp to maintain correct press/release semantics.
/// `down`: true for key press, false for key release.
pub(super) fn input_char_wayland_key_event(chr: char, down: bool, keyboard: &mut VirtualDevice) {
    let keysym = char_to_keysym(chr);
    let portal_state: u32 = if down { 1 } else { 0 };

    if can_input_via_keysym(chr, keysym) {
        let portal_info = {
            let session_info = RDP_SESSION_INFO.lock().unwrap();
            session_info
                .as_ref()
                .map(|info| (info.conn.clone(), info.session.clone()))
        };
        if let Some((ref conn, ref session)) = portal_info {
            let portal = scrap::wayland::pipewire::get_portal(conn);
            if portal
                .notify_keyboard_keysym(session, HashMap::new(), keysym, portal_state)
                .is_ok()
            {
                return;
            }
        }
        // Portal unavailable or failed, fallback to uinput
        let key = enigo::Key::Layout(chr);
        if let Ok((evdev_key, is_shift)) = map_key(&key) {
            if down {
                // Press: Shift↓ (if needed) → Key↓
                if is_shift {
                    let shift_down =
                        InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 1);
                    if let Err(e) = keyboard.emit(&[shift_down]) {
                        log::warn!("input_char_wayland_key_event: failed to press Shift for '{}': {:?}", chr, e);
                    }
                }
                let key_down = InputEvent::new(EventType::KEY, evdev_key.code(), 1);
                allow_err!(keyboard.emit(&[key_down]));
            } else {
                // Release: Key↑ → Shift↑ (if needed)
                let key_up = InputEvent::new(EventType::KEY, evdev_key.code(), 0);
                allow_err!(keyboard.emit(&[key_up]));
                if is_shift {
                    let shift_up =
                        InputEvent::new(EventType::KEY, evdev::Key::KEY_LEFTSHIFT.code(), 0);
                    if let Err(e) = keyboard.emit(&[shift_up]) {
                        log::warn!("input_char_wayland_key_event: failed to release Shift for '{}': {:?}", chr, e);
                    }
                }
            }
        }
    } else {
        // Non-ASCII: no reliable down/up semantics available.
        // Clipboard paste is atomic and handled elsewhere.
        log::debug!(
            "Skipping non-ASCII character key {} in uinput service",
            if down { "down" } else { "up" }
        );
    }
}

/// Check if character can be input via keysym (ASCII printable with valid keysym).
#[inline]
pub(crate) fn can_input_via_keysym(c: char, keysym: i32) -> bool {
    // ASCII printable: 0x20 (space) to 0x7E (tilde)
    (c as u32 >= 0x20 && c as u32 <= 0x7E) && keysym != 0
}

/// Convert a Unicode character to X11 keysym.
pub(crate) fn char_to_keysym(c: char) -> i32 {
    let codepoint = c as u32;
    if codepoint == 0 {
        // Null character has no keysym
        0
    } else if (0x20..=0x7E).contains(&codepoint) {
        // ASCII printable (0x20-0x7E): keysym == Unicode codepoint
        codepoint as i32
    } else if (0xA0..=0xFF).contains(&codepoint) {
        // Latin-1 supplement (0xA0-0xFF): keysym == Unicode codepoint (per X11 keysym spec)
        codepoint as i32
    } else {
        // Everything else (control chars 0x01-0x1F, DEL 0x7F, and all other non-ASCII Unicode):
        // keysym = 0x01000000 | codepoint (X11 Unicode keysym encoding)
        (0x0100_0000 | codepoint) as i32
    }
}
