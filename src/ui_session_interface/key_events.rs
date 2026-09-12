use super::*;

impl<T: InvokeUiSession> Session<T> {
    pub fn lock_screen(&self) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        self.send_key_event(&crate::keyboard::client::event_lock_screen());
    }
    pub fn ctrl_alt_del(&self) {
        self.send_key_event(&crate::keyboard::client::event_ctrl_alt_del());
    }
}

impl<T: InvokeUiSession> Session<T> {
    pub fn swap_modifier_key(&self, msg: &mut KeyEvent) {
        let allow_swap_key = self.get_toggle_option("allow_swap_key".to_string());
        if allow_swap_key {
            if let Some(key_event::Union::ControlKey(ck)) = msg.union {
                let ck = ck.enum_value_or_default();
                let ck = match ck {
                    ControlKey::Control => ControlKey::Meta,
                    ControlKey::Meta => ControlKey::Control,
                    ControlKey::RControl => ControlKey::Meta,
                    ControlKey::RWin => ControlKey::Control,
                    _ => ck,
                };
                msg.set_control_key(ck);
            }
            msg.modifiers = msg
                .modifiers
                .iter()
                .map(|ck| {
                    let ck = ck.enum_value_or_default();
                    let ck = match ck {
                        ControlKey::Control => ControlKey::Meta,
                        ControlKey::Meta => ControlKey::Control,
                        ControlKey::RControl => ControlKey::Meta,
                        ControlKey::RWin => ControlKey::Control,
                        _ => ck,
                    };
                    hbb_common::protobuf::EnumOrUnknown::new(ck)
                })
                .collect();

            let code = msg.chr();
            if code != 0 {
                let mut peer = self.peer_platform().to_lowercase();
                peer.retain(|c| !c.is_whitespace());

                let key = match peer.as_str() {
                    "windows" => {
                        let key = rdev::win_key_from_scancode(code);
                        let key = match key {
                            rdev::Key::ControlLeft => rdev::Key::MetaLeft,
                            rdev::Key::MetaLeft => rdev::Key::ControlLeft,
                            rdev::Key::ControlRight => rdev::Key::MetaLeft,
                            rdev::Key::MetaRight => rdev::Key::ControlLeft,
                            _ => key,
                        };
                        rdev::win_scancode_from_key(key).unwrap_or_default()
                    }
                    "macos" => {
                        let key = rdev::macos_key_from_code(code as _);
                        let key = match key {
                            rdev::Key::ControlLeft => rdev::Key::MetaLeft,
                            rdev::Key::MetaLeft => rdev::Key::ControlLeft,
                            rdev::Key::ControlRight => rdev::Key::MetaLeft,
                            rdev::Key::MetaRight => rdev::Key::ControlLeft,
                            _ => key,
                        };
                        rdev::macos_keycode_from_key(key).unwrap_or_default() as _
                    }
                    _ => {
                        let key = rdev::linux_key_from_code(code);
                        let key = match key {
                            rdev::Key::ControlLeft => rdev::Key::MetaLeft,
                            rdev::Key::MetaLeft => rdev::Key::ControlLeft,
                            rdev::Key::ControlRight => rdev::Key::MetaLeft,
                            rdev::Key::MetaRight => rdev::Key::ControlLeft,
                            _ => key,
                        };
                        rdev::linux_keycode_from_key(key).unwrap_or_default()
                    }
                };
                msg.set_chr(key);
            }
        }
    }

    pub fn send_key_event(&self, evt: &KeyEvent) {
        // mode: legacy(0), map(1), translate(2), auto(3)

        let mut msg = evt.clone();
        self.swap_modifier_key(&mut msg);
        let mut msg_out = Message::new();
        msg_out.set_key_event(msg);
        self.send(Data::Message(msg_out));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn enter(&self, keyboard_mode: String) {
        let session_id = self.lc.read().unwrap().session_id as u128;
        keyboard::client::change_grab_status(GrabState::Run, &keyboard_mode, session_id);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn leave(&self, keyboard_mode: String) {
        let session_id = self.lc.read().unwrap().session_id as u128;
        keyboard::client::change_grab_status(GrabState::Wait, &keyboard_mode, session_id);
    }

    // flutter only TODO new input
    pub fn input_key(
        &self,
        name: &str,
        down: bool,
        press: bool,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let chars: Vec<char> = name.chars().collect();
        if chars.len() == 1 {
            let key = Key::_Raw(chars[0] as _);
            self._input_key(key, down, press, alt, ctrl, shift, command);
        } else {
            if let Some(key) = KEY_MAP.get(name) {
                self._input_key(key.clone(), down, press, alt, ctrl, shift, command);
            }
        }
    }

    pub fn input_string(&self, value: &str) {
        let mut key_event = KeyEvent::new();
        key_event.set_seq(value.to_owned());
        let mut msg_out = Message::new();
        msg_out.set_key_event(key_event);
        self.send(Data::Message(msg_out));
    }

    #[cfg(any(target_os = "ios"))]
    pub fn handle_flutter_raw_key_event(
        &self,
        _keyboard_mode: &str,
        _name: &str,
        _platform_code: i32,
        _position_code: i32,
        _lock_modes: i32,
        _down_or_up: bool,
    ) {
    }

    #[cfg(not(any(target_os = "ios")))]
    pub fn handle_flutter_raw_key_event(
        &self,
        keyboard_mode: &str,
        name: &str,
        platform_code: i32,
        position_code: i32,
        lock_modes: i32,
        down_or_up: bool,
    ) {
        if name == "flutter_key" {
            self._handle_key_flutter_simulation(keyboard_mode, platform_code, down_or_up);
        } else {
            self._handle_raw_key_non_flutter_simulation(
                keyboard_mode,
                platform_code,
                position_code,
                lock_modes,
                down_or_up,
            );
        }
    }
}
