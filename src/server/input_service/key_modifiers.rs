use super::*;

#[inline]
pub(super) fn control_key_value_to_key(value: i32) -> Option<Key> {
    KEY_MAP.get(&value).and_then(|k| Some(*k))
}

#[inline]
pub(super) fn char_value_to_key(value: u32) -> Key {
    Key::Layout(std::char::from_u32(value).unwrap_or('\0'))
}

pub(super) fn map_keyboard_mode(evt: &KeyEvent) {
    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();

    // Wayland
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() {
        wayland_send_raw_key(evt.chr() as u16, evt.down);
        return;
    }

    sim_rdev_rawkey_position(evt.chr() as _, evt.down);
}

/// Send raw keycode on Wayland via the active backend (uinput or RemoteDesktop portal).
/// The keycode is expected to be a Linux keycode (evdev code + 8 for X11 compatibility).
#[cfg(target_os = "linux")]
#[inline]
pub(super) fn wayland_send_raw_key(code: u16, down: bool) {
    let mut en = ENIGO.lock().unwrap();
    if down {
        en.key_down(enigo::Key::Raw(code)).ok();
    } else {
        en.key_up(enigo::Key::Raw(code));
    }
}

#[cfg(target_os = "macos")]
pub(super) fn add_flags_to_enigo(en: &mut Enigo, key_event: &KeyEvent) {
    // When long-pressed the command key, then press and release
    // the Tab key, there should be CGEventFlagCommand in the flag.
    en.reset_flag();
    for ck in key_event.modifiers.iter() {
        if let Some(key) = KEY_MAP.get(&ck.value()) {
            en.add_flag(key);
        }
    }
}

pub(super) fn get_control_key_value(key_event: &KeyEvent) -> i32 {
    if let Some(key_event::Union::ControlKey(ck)) = key_event.union {
        ck.value()
    } else {
        -1
    }
}

#[inline]
pub(super) fn has_hotkey_modifiers(key_event: &KeyEvent) -> bool {
    key_event.modifiers.iter().any(|ck| {
        let v = ck.value();
        v == ControlKey::Control.value()
            || v == ControlKey::RControl.value()
            || v == ControlKey::Meta.value()
            || v == ControlKey::RWin.value()
            || {
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                {
                    v == ControlKey::Alt.value() || v == ControlKey::RAlt.value()
                }
                #[cfg(target_os = "macos")]
                {
                    false
                }
            }
    })
}

pub(super) fn release_unpressed_modifiers(en: &mut Enigo, key_event: &KeyEvent) {
    let ck_value = get_control_key_value(key_event);
    fix_modifiers(&key_event.modifiers[..], en, ck_value);
}

#[cfg(target_os = "linux")]
pub(super) fn is_altgr_pressed() -> bool {
    let altgr_rawkey = RawKey::LinuxXorgKeycode(ControlKey::RAlt.value() as _);
    KEYS_DOWN
        .lock()
        .unwrap()
        .get(&KeysDown::RdevKey(altgr_rawkey))
        .is_some()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn press_modifiers(en: &mut Enigo, key_event: &KeyEvent, to_release: &mut Vec<Key>) {
    for ref ck in key_event.modifiers.iter() {
        if let Some(key) = control_key_value_to_key(ck.value()) {
            if !is_pressed(&key, en) {
                #[cfg(target_os = "linux")]
                if key == Key::Alt && is_altgr_pressed() {
                    continue;
                }
                en.key_down(key.clone()).ok();
                to_release.push(key.clone());
                #[cfg(windows)]
                modifier_sleep();
            }
        }
    }
}

pub(super) fn sync_modifiers(en: &mut Enigo, key_event: &KeyEvent, _to_release: &mut Vec<Key>) {
    #[cfg(target_os = "macos")]
    add_flags_to_enigo(en, key_event);

    if key_event.down {
        release_unpressed_modifiers(en, key_event);
        #[cfg(not(target_os = "macos"))]
        press_modifiers(en, key_event, _to_release);
    }
}

pub(super) fn process_control_key(en: &mut Enigo, ck: &EnumOrUnknown<ControlKey>, down: bool) {
    if let Some(key) = control_key_value_to_key(ck.value()) {
        if down {
            en.key_down(key).ok();
        } else {
            en.key_up(key);
        }
    }
}

#[inline]
pub(super) fn need_to_uppercase(en: &mut Enigo) -> bool {
    get_modifier_state(Key::Shift, en) || get_modifier_state(Key::CapsLock, en)
}

pub(super) fn process_chr(en: &mut Enigo, chr: u32, down: bool, _hotkey: bool) {
    // On Wayland with uinput mode:
    // - ASCII printable: input via key events (custom keyboard path, e.g. portal keysym)
    // - Non-ASCII: input via clipboard paste
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() && wayland_use_uinput() {
        // Skip clipboard for hotkeys (Ctrl/Alt/Meta pressed)
        if !is_hotkey_modifier_pressed(en) {
            if let Ok(c) = char::try_from(chr) {
                if is_ascii_printable(c) {
                    if down {
                        en.key_down(Key::Layout(c)).ok();
                    } else {
                        en.key_up(Key::Layout(c));
                    }
                } else if down {
                    input_char_via_clipboard_server(en, c);
                }
            } else {
                log::warn!(
                    "Ignore invalid unicode scalar in Wayland+uinput path: {}",
                    chr
                );
            }
            return;
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if !_hotkey {
        if down {
            if let Ok(chr) = char::try_from(chr) {
                en.key_sequence(&chr.to_string());
            }
        }
        return;
    }

    let key = char_value_to_key(chr);

    if down {
        if en.key_down(key).is_ok() {
        } else {
            if let Ok(chr) = char::try_from(chr) {
                let mut s = chr.to_string();
                if need_to_uppercase(en) {
                    s = s.to_uppercase();
                }
                en.key_sequence(&s);
            };
        }
    } else {
        en.key_up(key);
    }
}

pub(super) fn process_unicode(en: &mut Enigo, chr: u32) {
    // On Wayland with uinput mode:
    // - ASCII printable: input via key sequence (custom keyboard path)
    // - Non-ASCII: input via clipboard paste
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() && wayland_use_uinput() {
        if let Ok(c) = char::try_from(chr) {
            if is_ascii_printable(c) {
                en.key_sequence(&c.to_string());
            } else {
                input_char_via_clipboard_server(en, c);
            }
        }
        return;
    }

    if let Ok(chr) = char::try_from(chr) {
        en.key_sequence(&chr.to_string());
    }
}

pub(super) fn process_seq(en: &mut Enigo, sequence: &str) {
    // On Wayland with uinput mode:
    // - pure ASCII printable sequence: input via key sequence (custom keyboard path)
    // - any non-ASCII present: input whole sequence via clipboard to preserve order
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() && wayland_use_uinput() {
        if sequence.chars().all(is_ascii_printable) {
            en.key_sequence(sequence);
        } else {
            input_text_via_clipboard_server(en, sequence);
        }
        return;
    }

    en.key_sequence(&sequence);
}
