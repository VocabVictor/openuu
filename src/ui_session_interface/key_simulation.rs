use super::*;

impl<T: InvokeUiSession> Session<T> {
    #[cfg(not(any(target_os = "ios")))]
    pub(super) fn _handle_raw_key_non_flutter_simulation(
        &self,
        keyboard_mode: &str,
        platform_code: i32,
        position_code: i32,
        lock_modes: i32,
        down_or_up: bool,
    ) {
        if position_code < 0 || platform_code < 0 {
            return;
        }
        let platform_code: u32 = platform_code as _;
        let position_code: KeyCode = position_code as _;

        #[cfg(not(target_os = "windows"))]
        let key = rdev::key_from_code(position_code) as rdev::Key;
        // Windows requires special handling
        #[cfg(target_os = "windows")]
        let key = rdev::get_win_key(platform_code, position_code);

        let event_type = if down_or_up {
            KeyPress(key)
        } else {
            KeyRelease(key)
        };
        let event = Event {
            time: SystemTime::now(),
            unicode: None,
            platform_code,
            position_code: position_code as _,
            event_type,
            usb_hid: 0,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        };
        keyboard::client::process_event_with_session(keyboard_mode, &event, Some(lock_modes), self);
    }

    pub fn handle_flutter_key_event(
        &self,
        keyboard_mode: &str,
        character: &str,
        usb_hid: i32,
        lock_modes: i32,
        down_or_up: bool,
    ) {
        if character == "flutter_key" {
            self._handle_key_flutter_simulation(keyboard_mode, usb_hid, down_or_up);
        } else {
            self._handle_key_non_flutter_simulation(
                keyboard_mode,
                character,
                usb_hid,
                lock_modes,
                down_or_up,
            );
        }
    }

    pub(super) fn _handle_key_flutter_simulation(
        &self,
        _keyboard_mode: &str,
        platform_code: i32,
        down_or_up: bool,
    ) {
        // https://github.com/flutter/flutter/blob/master/packages/flutter/lib/src/services/keyboard_key.g.dart#L4356
        let ctrl_key = match platform_code {
            0x007f => Some(ControlKey::VolumeMute),
            0x0080 => Some(ControlKey::VolumeUp),
            0x0081 => Some(ControlKey::VolumeDown),
            0x0066 => Some(ControlKey::Power),
            _ => None,
        };
        let Some(ctrl_key) = ctrl_key else { return };
        let mut key_event = KeyEvent {
            mode: KeyboardMode::Translate.into(),
            down: down_or_up,
            ..Default::default()
        };
        key_event.set_control_key(ctrl_key);
        self.send_key_event(&key_event);
    }

    pub(super) fn _handle_key_non_flutter_simulation(
        &self,
        keyboard_mode: &str,
        character: &str,
        usb_hid: i32,
        lock_modes: i32,
        down_or_up: bool,
    ) {
        let key = rdev::usb_hid_key_from_code(usb_hid as _);

        #[cfg(any(target_os = "android", target_os = "ios"))]
        let position_code: KeyCode = 0;
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let platform_code: KeyCode = 0;

        #[cfg(target_os = "windows")]
        let platform_code: u32 = rdev::win_code_from_key(key).unwrap_or(0);
        #[cfg(target_os = "windows")]
        let position_code: KeyCode = rdev::win_scancode_from_key(key).unwrap_or(0) as _;

        #[cfg(not(any(target_os = "windows", target_os = "android", target_os = "ios")))]
        let position_code: KeyCode = rdev::code_from_key(key).unwrap_or(0) as _;
        #[cfg(not(any(
            target_os = "windows",
            target_os = "android",
            target_os = "ios",
            target_os = "linux"
        )))]
        let platform_code: u32 = position_code as _;
        // For translate mode.
        // We need to set the platform code (keysym) if is AltGr.
        // https://github.com/rustdesk/rustdesk/blob/07cf1b4db5ef2f925efd3b16b87c33ce03c94809/src/keyboard.rs#L1029
        // https://github.com/flutter/flutter/issues/153811
        #[cfg(target_os = "linux")]
        let platform_code: u32 = position_code as _;

        let event_type = if down_or_up {
            KeyPress(key)
        } else {
            KeyRelease(key)
        };
        let event = Event {
            time: SystemTime::now(),
            unicode: if character.is_empty() {
                None
            } else {
                Some(rdev::UnicodeInfo {
                    name: Some(character.to_string()),
                    unicode: character.encode_utf16().collect(),
                    // is_dead: is not correct here, because flutter cannot detect deadcode for now.
                    is_dead: false,
                })
            },
            platform_code,
            position_code: position_code as _,
            event_type,
            #[cfg(any(target_os = "android", target_os = "ios"))]
            usb_hid: usb_hid as _,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            usb_hid: 0,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        };
        keyboard::client::process_event_with_session(keyboard_mode, &event, Some(lock_modes), self);
    }

    // flutter only TODO new input
    pub(super) fn _input_key(
        &self,
        key: Key,
        down: bool,
        press: bool,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let v = if press {
            3
        } else if down {
            1
        } else {
            0
        };
        let mut key_event = KeyEvent::new();
        match key {
            Key::Chr(chr) => {
                key_event.set_chr(chr);
            }
            Key::ControlKey(key) => {
                key_event.set_control_key(key.clone());
            }
            Key::_Raw(raw) => {
                key_event.set_chr(raw);
            }
        }

        if v == 1 {
            key_event.down = true;
        } else if v == 3 {
            key_event.press = true;
        }
        keyboard::client::legacy_modifiers(&mut key_event, alt, ctrl, shift, command);
        key_event.mode = KeyboardMode::Legacy.into();

        self.send_key_event(&key_event);
    }
}
