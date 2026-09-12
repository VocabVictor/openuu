use super::*;

pub(super) fn legacy_keyboard_mode(evt: &KeyEvent) {
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let mut to_release: Vec<Key> = Vec::new();

    let mut en = ENIGO.lock().unwrap();
    sync_modifiers(&mut en, &evt, &mut to_release);

    let down = evt.down;
    match evt.union {
        Some(key_event::Union::ControlKey(ck)) => {
            if is_function_key(&ck) {
                return;
            }
            let record_key = ck.value() as u64;
            record_pressed_key(KeysDown::EnigoKey(record_key), down);
            process_control_key(&mut en, &ck, down)
        }
        Some(key_event::Union::Chr(chr)) => {
            // For character input in Legacy mode, we need to release Shift first.
            // The character has already been converted by the client, so we should
            // input it directly without Shift modifier affecting the result.
            // Only Ctrl/Alt/Meta should be kept for hotkeys like Ctrl+C.
            #[cfg(target_os = "linux")]
            release_shift_for_char_input(&mut en);

            let record_key = chr as u64 + KEY_CHAR_START;
            record_pressed_key(KeysDown::EnigoKey(record_key), down);
            process_chr(&mut en, chr, down, has_hotkey_modifiers(evt))
        }
        Some(key_event::Union::Unicode(chr)) => {
            // Same as Chr: release Shift for Unicode input
            #[cfg(target_os = "linux")]
            release_shift_for_char_input(&mut en);

            process_unicode(&mut en, chr)
        }
        Some(key_event::Union::Seq(ref seq)) => process_seq(&mut en, seq),
        _ => {}
    }

    #[cfg(not(target_os = "macos"))]
    release_keys(&mut en, &to_release);
}

#[cfg(target_os = "windows")]
pub(super) fn translate_process_code(code: u32, down: bool) {
    crate::platform::windows::try_change_desktop();
    match code >> 16 {
        0 => sim_rdev_rawkey_position(code as _, down),
        vk_code => sim_rdev_rawkey_virtual(vk_code, down),
    };
}

pub(super) fn translate_keyboard_mode(evt: &KeyEvent) {
    match &evt.union {
        Some(key_event::Union::Seq(seq)) => {
            // On Wayland:
            // - uinput mode (--service): keep clipboard handling in this process because
            //   clipboard is unreliable in root service context.
            // - rdp_input mode (--server): forward sequence to custom keyboard handler so
            //   ASCII can use Portal keysym and non-ASCII can use clipboard.
            #[cfg(target_os = "linux")]
            if !crate::platform::linux::is_x11() {
                let mut en = ENIGO.lock().unwrap();
                if wayland_use_rdp_input() {
                    release_shift_for_char_input(&mut en);
                    en.key_sequence(seq);
                    return;
                }

                if wayland_use_uinput() {
                    // Check if this is a hotkey (Ctrl/Alt/Meta pressed)
                    // For hotkeys, we send character-based key events via Enigo instead of
                    // using the clipboard. This relies on the local keyboard layout for
                    // mapping characters to physical keys.
                    // This assumes client and server use the same keyboard layout (common case).
                    // Note: For non-Latin keyboards (e.g., Arabic), hotkeys may not work
                    // correctly if the character cannot be mapped to a key via KEY_MAP_LAYOUT.
                    // This is a known limitation - most common hotkeys (Ctrl+A/C/V/Z) use Latin
                    // characters which are mappable on most keyboard layouts.
                    if is_hotkey_modifier_pressed(&mut en) {
                        // For hotkeys, send character-based key events via Enigo.
                        // This relies on the local keyboard layout mapping (KEY_MAP_LAYOUT).
                        for chr in seq.chars() {
                            if !is_ascii_printable(chr) {
                                log::warn!(
                                    "Hotkey with non-ASCII character may not work correctly on non-Latin keyboard layouts"
                                );
                            }
                            en.key_click(Key::Layout(chr));
                        }
                        return;
                    }

                    // Normal text input: release Shift and use clipboard
                    release_shift_for_char_input(&mut en);
                    if seq.chars().all(is_ascii_printable) {
                        en.key_sequence(seq);
                    } else {
                        input_text_via_clipboard_server(&mut en, seq);
                    }
                    return;
                }
            }

            // Fr -> US
            // client: Shift + & => 1(send to remote)
            // remote: Shift + 1 => !
            //
            // Try to release shift first.
            // remote: Shift + 1 => 1
            let mut en = ENIGO.lock().unwrap();

            #[cfg(target_os = "macos")]
            en.key_sequence(seq);
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            {
                #[cfg(target_os = "windows")]
                let simulate_win_hot_key = is_hot_key_modifiers_down(&mut en);
                #[cfg(target_os = "linux")]
                let simulate_win_hot_key = false;
                if !simulate_win_hot_key {
                    #[cfg(target_os = "linux")]
                    release_shift_for_char_input(&mut en);
                    #[cfg(target_os = "windows")]
                    {
                        if get_modifier_state(Key::Shift, &mut en) {
                            simulate_(&EventType::KeyRelease(RdevKey::ShiftLeft));
                        }
                        if get_modifier_state(Key::RightShift, &mut en) {
                            simulate_(&EventType::KeyRelease(RdevKey::ShiftRight));
                        }
                    }
                }
                for chr in seq.chars() {
                    // char in rust is 4 bytes.
                    // But for this case, char comes from keyboard. We only need 2 bytes.
                    #[cfg(target_os = "windows")]
                    if simulate_win_hot_key {
                        rdev::simulate_char(chr, true).ok();
                    } else {
                        rdev::simulate_unicode(chr as _).ok();
                    }
                    #[cfg(target_os = "linux")]
                    en.key_click(Key::Layout(chr));
                }
            }
        }
        Some(key_event::Union::Chr(..)) => {
            #[cfg(target_os = "windows")]
            translate_process_code(evt.chr(), evt.down);
            #[cfg(target_os = "linux")]
            {
                if !crate::platform::linux::is_x11() {
                    // Wayland: use uinput to send raw keycode
                    wayland_send_raw_key(evt.chr() as u16, evt.down);
                } else {
                    sim_rdev_rawkey_position(evt.chr() as _, evt.down);
                }
            }
            #[cfg(target_os = "macos")]
            sim_rdev_rawkey_position(evt.chr() as _, evt.down);
        }
        Some(key_event::Union::Unicode(..)) => {
            // Do not handle unicode for now.
        }
        #[cfg(target_os = "windows")]
        Some(key_event::Union::Win2winHotkey(code)) => {
            simulate_win2win_hotkey(*code, evt.down);
        }
        _ => {
            log::debug!(
                "Unreachable. Unexpected key event (mode={:?}, down={:?})",
                &evt.mode,
                &evt.down
            );
        }
    }
}

#[inline]
#[cfg(target_os = "windows")]
pub(super) fn is_hot_key_modifiers_down(en: &mut Enigo) -> bool {
    en.get_key_state(Key::Control)
        || en.get_key_state(Key::RightControl)
        || en.get_key_state(Key::Alt)
        || en.get_key_state(Key::RightAlt)
        || en.get_key_state(Key::Meta)
        || en.get_key_state(Key::RWin)
}

#[cfg(target_os = "windows")]
pub(super) fn simulate_win2win_hotkey(code: u32, down: bool) {
    let unicode: u16 = (code & 0x0000FFFF) as u16;
    if down {
        if rdev::simulate_key_unicode(unicode, false).is_ok() {
            return;
        }
    }

    let keycode: u16 = ((code >> 16) & 0x0000FFFF) as u16;
    let scan = rdev::vk_to_scancode(keycode as _);
    allow_err!(rdev::simulate_code(None, Some(scan), down));
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub(super) fn skip_led_sync_control_key(_key: &ControlKey) -> bool {
    false
}

// LockModesHandler should not be created when single meta is pressing and releasing.
// Because the drop function may insert "CapsLock Click" and "NumLock Click", which breaks single meta click.
// https://github.com/rustdesk/rustdesk/issues/3928#issuecomment-1496936687
// https://github.com/rustdesk/rustdesk/issues/3928#issuecomment-1500415822
// https://github.com/rustdesk/rustdesk/issues/3928#issuecomment-1500773473
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(super) fn skip_led_sync_control_key(key: &ControlKey) -> bool {
    matches!(
        key,
        ControlKey::Control
            | ControlKey::RControl
            | ControlKey::Meta
            | ControlKey::Shift
            | ControlKey::RShift
            | ControlKey::Alt
            | ControlKey::RAlt
            | ControlKey::Tab
            | ControlKey::Return
    )
}

#[inline]
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(super) fn is_numpad_control_key(key: &ControlKey) -> bool {
    matches!(
        key,
        ControlKey::Numpad0
            | ControlKey::Numpad1
            | ControlKey::Numpad2
            | ControlKey::Numpad3
            | ControlKey::Numpad4
            | ControlKey::Numpad5
            | ControlKey::Numpad6
            | ControlKey::Numpad7
            | ControlKey::Numpad8
            | ControlKey::Numpad9
            | ControlKey::NumpadEnter
    )
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub(super) fn skip_led_sync_rdev_key(_key: &RdevKey) -> bool {
    false
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(super) fn skip_led_sync_rdev_key(key: &RdevKey) -> bool {
    matches!(
        key,
        RdevKey::ControlLeft
            | RdevKey::ControlRight
            | RdevKey::MetaLeft
            | RdevKey::MetaRight
            | RdevKey::ShiftLeft
            | RdevKey::ShiftRight
            | RdevKey::Alt
            | RdevKey::AltGr
            | RdevKey::Tab
            | RdevKey::Return
    )
}
