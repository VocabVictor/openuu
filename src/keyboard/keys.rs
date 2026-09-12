use super::*;

pub fn is_long_press(event: &Event) -> bool {
    let keys = MODIFIERS_STATE.lock().unwrap();
    match event.event_type {
        EventType::KeyPress(k) => {
            if let Some(&state) = keys.get(&k) {
                if state == true {
                    return true;
                }
            }
        }
        _ => {}
    };
    return false;
}

pub(super) fn take_remote_keys() -> HashMap<Key, Event> {
    let mut to_release = TO_RELEASE.lock().unwrap();
    std::mem::take(&mut *to_release)
}

pub(super) fn release_remote_keys_for_events(keyboard_mode: &str, to_release: HashMap<Key, Event>) {
    for (key, mut event) in to_release.into_iter() {
        event.event_type = EventType::KeyRelease(key);
        client::process_event(keyboard_mode, &event, None);
        // If Alt or AltGr is pressed, we need to send another key stoke to release it.
        // Because the controlled side may hold the alt state, if local window is switched by [Alt + Tab].
        if key == Key::Alt || key == Key::AltGr {
            event.event_type = EventType::KeyPress(key);
            client::process_event(keyboard_mode, &event, None);
            event.event_type = EventType::KeyRelease(key);
            client::process_event(keyboard_mode, &event, None);
        }
    }
}

#[allow(dead_code)]
pub fn release_remote_keys(keyboard_mode: &str) {
    // todo!: client quit suddenly, how to release keys?
    release_remote_keys_for_events(keyboard_mode, take_remote_keys());
}

pub fn get_keyboard_mode_enum(keyboard_mode: &str) -> KeyboardMode {
    match keyboard_mode {
        "map" => KeyboardMode::Map,
        "translate" => KeyboardMode::Translate,
        "legacy" => KeyboardMode::Legacy,
        _ => KeyboardMode::Map,
    }
}

#[inline]
pub fn is_modifier(key: &rdev::Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::MetaLeft
            | Key::MetaRight
            | Key::Alt
            | Key::AltGr
    )
}

#[inline]
#[allow(dead_code)]
pub fn is_modifier_code(evt: &KeyEvent) -> bool {
    match evt.union {
        Some(key_event::Union::Chr(code)) => {
            let key = rdev::linux_key_from_code(code);
            is_modifier(&key)
        }
        _ => false,
    }
}

#[inline]
pub fn is_numpad_rdev_key(key: &rdev::Key) -> bool {
    matches!(
        key,
        Key::Kp0
            | Key::Kp1
            | Key::Kp2
            | Key::Kp3
            | Key::Kp4
            | Key::Kp5
            | Key::Kp6
            | Key::Kp7
            | Key::Kp8
            | Key::Kp9
            | Key::KpMinus
            | Key::KpMultiply
            | Key::KpDivide
            | Key::KpPlus
            | Key::KpDecimal
    )
}

#[inline]
pub fn is_letter_rdev_key(key: &rdev::Key) -> bool {
    matches!(
        key,
        Key::KeyA
            | Key::KeyB
            | Key::KeyC
            | Key::KeyD
            | Key::KeyE
            | Key::KeyF
            | Key::KeyG
            | Key::KeyH
            | Key::KeyI
            | Key::KeyJ
            | Key::KeyK
            | Key::KeyL
            | Key::KeyM
            | Key::KeyN
            | Key::KeyO
            | Key::KeyP
            | Key::KeyQ
            | Key::KeyR
            | Key::KeyS
            | Key::KeyT
            | Key::KeyU
            | Key::KeyV
            | Key::KeyW
            | Key::KeyX
            | Key::KeyY
            | Key::KeyZ
    )
}

// https://github.com/rustdesk/rustdesk/issues/8599
// We just add these keys as letter keys.
#[inline]
pub fn is_letter_rdev_key_ex(key: &rdev::Key) -> bool {
    matches!(
        key,
        Key::LeftBracket | Key::RightBracket | Key::SemiColon | Key::Quote | Key::Comma | Key::Dot
    )
}

#[inline]
pub(super) fn is_numpad_key(event: &Event) -> bool {
    matches!(event.event_type, EventType::KeyPress(key) | EventType::KeyRelease(key) if is_numpad_rdev_key(&key))
}

// Check is letter key for lock modes.
// Only letter keys need to check and send Lock key state.
#[inline]
pub(super) fn is_letter_key_4_lock_modes(event: &Event) -> bool {
    matches!(event.event_type, EventType::KeyPress(key) | EventType::KeyRelease(key) if (is_letter_rdev_key(&key) || is_letter_rdev_key_ex(&key)))
}

pub(super) fn parse_add_lock_modes_modifiers(
    key_event: &mut KeyEvent,
    lock_modes: i32,
    is_numpad_key: bool,
    is_letter_key: bool,
) {
    const CAPS_LOCK: i32 = 1;
    const NUM_LOCK: i32 = 2;
    // const SCROLL_LOCK: i32 = 3;
    if is_letter_key && (lock_modes & (1 << CAPS_LOCK) != 0) {
        key_event.modifiers.push(ControlKey::CapsLock.into());
    }
    if is_numpad_key && lock_modes & (1 << NUM_LOCK) != 0 {
        key_event.modifiers.push(ControlKey::NumLock.into());
    }
    // if lock_modes & (1 << SCROLL_LOCK) != 0 {
    //     key_event.modifiers.push(ControlKey::ScrollLock.into());
    // }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn add_lock_modes_modifiers(key_event: &mut KeyEvent, is_numpad_key: bool, is_letter_key: bool) {
    if is_letter_key && get_key_state(enigo::Key::CapsLock) {
        key_event.modifiers.push(ControlKey::CapsLock.into());
    }
    if is_numpad_key && get_key_state(enigo::Key::NumLock) {
        key_event.modifiers.push(ControlKey::NumLock.into());
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn convert_numpad_keys(key: Key) -> Key {
    if get_key_state(enigo::Key::NumLock) {
        return key;
    }
    match key {
        Key::Kp0 => Key::Insert,
        Key::KpDecimal => Key::Delete,
        Key::Kp1 => Key::End,
        Key::Kp2 => Key::DownArrow,
        Key::Kp3 => Key::PageDown,
        Key::Kp4 => Key::LeftArrow,
        Key::Kp5 => Key::Clear,
        Key::Kp6 => Key::RightArrow,
        Key::Kp7 => Key::Home,
        Key::Kp8 => Key::UpArrow,
        Key::Kp9 => Key::PageUp,
        _ => key,
    }
}

pub(super) fn update_modifiers_state(event: &Event) {
    // for mouse
    let mut keys = MODIFIERS_STATE.lock().unwrap();
    match event.event_type {
        EventType::KeyPress(k) => {
            if keys.contains_key(&k) {
                keys.insert(k, true);
            }
        }
        EventType::KeyRelease(k) => {
            if keys.contains_key(&k) {
                keys.insert(k, false);
            }
        }
        _ => {}
    };
}
