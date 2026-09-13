use super::*;

/// Modifier key state tracking for RDP input.
/// Portal API doesn't provide a way to query key state, so we track it ourselves.
#[derive(Default)]
pub(super) struct ModifierState {
    pub(super) shift_left: bool,
    pub(super) shift_right: bool,
    pub(super) ctrl_left: bool,
    pub(super) ctrl_right: bool,
    pub(super) alt_left: bool,
    pub(super) alt_right: bool,
    pub(super) meta_left: bool,
    pub(super) meta_right: bool,
}

impl ModifierState {
    pub(super) fn update(&mut self, key: &Key, down: bool) {
        match key {
            Key::Shift => self.shift_left = down,
            Key::RightShift => self.shift_right = down,
            Key::Control => self.ctrl_left = down,
            Key::RightControl => self.ctrl_right = down,
            Key::Alt => self.alt_left = down,
            Key::RightAlt => self.alt_right = down,
            Key::Meta | Key::Super | Key::Windows | Key::Command => self.meta_left = down,
            Key::RWin => self.meta_right = down,
            // Handle raw keycodes for modifier keys (Linux evdev codes + 8)
            // In translate mode, modifier keys may be sent as Chr events with raw keycodes.
            // The +8 offset converts evdev codes to X11/XKB keycodes.
            Key::Raw(code) => {
                const EVDEV_OFFSET: u16 = 8;
                const KEY_LEFTSHIFT: u16 = evdev::Key::KEY_LEFTSHIFT.code() + EVDEV_OFFSET;
                const KEY_RIGHTSHIFT: u16 = evdev::Key::KEY_RIGHTSHIFT.code() + EVDEV_OFFSET;
                const KEY_LEFTCTRL: u16 = evdev::Key::KEY_LEFTCTRL.code() + EVDEV_OFFSET;
                const KEY_RIGHTCTRL: u16 = evdev::Key::KEY_RIGHTCTRL.code() + EVDEV_OFFSET;
                const KEY_LEFTALT: u16 = evdev::Key::KEY_LEFTALT.code() + EVDEV_OFFSET;
                const KEY_RIGHTALT: u16 = evdev::Key::KEY_RIGHTALT.code() + EVDEV_OFFSET;
                const KEY_LEFTMETA: u16 = evdev::Key::KEY_LEFTMETA.code() + EVDEV_OFFSET;
                const KEY_RIGHTMETA: u16 = evdev::Key::KEY_RIGHTMETA.code() + EVDEV_OFFSET;
                match *code {
                    KEY_LEFTSHIFT => self.shift_left = down,
                    KEY_RIGHTSHIFT => self.shift_right = down,
                    KEY_LEFTCTRL => self.ctrl_left = down,
                    KEY_RIGHTCTRL => self.ctrl_right = down,
                    KEY_LEFTALT => self.alt_left = down,
                    KEY_RIGHTALT => self.alt_right = down,
                    KEY_LEFTMETA => self.meta_left = down,
                    KEY_RIGHTMETA => self.meta_right = down,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

pub struct RdpInputKeyboard {
    pub(super) conn: Arc<SyncConnection>,
    pub(super) session: Path<'static>,
    pub(super) modifier_state: ModifierState,
}

impl RdpInputKeyboard {
    pub fn new(conn: Arc<SyncConnection>, session: Path<'static>) -> ResultType<Self> {
        Ok(Self {
            conn,
            session,
            modifier_state: ModifierState::default(),
        })
    }
}

impl KeyboardControllable for RdpInputKeyboard {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_key_state(&mut self, key: Key) -> bool {
        // Use tracked modifier state for supported keys
        match key {
            Key::Shift => self.modifier_state.shift_left,
            Key::RightShift => self.modifier_state.shift_right,
            Key::Control => self.modifier_state.ctrl_left,
            Key::RightControl => self.modifier_state.ctrl_right,
            Key::Alt => self.modifier_state.alt_left,
            Key::RightAlt => self.modifier_state.alt_right,
            Key::Meta | Key::Super | Key::Windows | Key::Command => {
                self.modifier_state.meta_left
            }
            Key::RWin => self.modifier_state.meta_right,
            _ => false,
        }
    }

    fn key_sequence(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }

        // Keep ordering deterministic:
        // - pure ASCII printable: send via Portal keysym
        // - any non-ASCII present (including mixed ASCII/non-ASCII): send whole
        //   sequence via clipboard as one atomic paste
        let ascii_only = s.chars().all(|c| {
            let keysym = char_to_keysym(c);
            can_input_via_keysym(c, keysym)
        });
        if !ascii_only {
            input_text_via_clipboard(s, self.conn.clone(), &self.session);
            return;
        }

        for c in s.chars() {
            let keysym = char_to_keysym(c);
            // ASCII characters: use keysym
            if can_input_via_keysym(c, keysym) {
                if let Err(e) = send_keysym(keysym, true, self.conn.clone(), &self.session) {
                    log::error!("Failed to send keysym down: {:?}", e);
                }
                if let Err(e) = send_keysym(keysym, false, self.conn.clone(), &self.session) {
                    log::error!("Failed to send keysym up: {:?}", e);
                }
            }
        }
    }

    fn key_down(&mut self, key: Key) -> enigo::ResultType {
        if let Key::Layout(chr) = key {
            let keysym = char_to_keysym(chr);
            // ASCII characters: use keysym
            if can_input_via_keysym(chr, keysym) {
                send_keysym(keysym, true, self.conn.clone(), &self.session)?;
            } else {
                // Non-ASCII: use clipboard (complete key press in key_down)
                input_text_via_clipboard(&chr.to_string(), self.conn.clone(), &self.session);
            }
        } else {
            handle_key(true, key.clone(), self.conn.clone(), &self.session)?;
            // Update modifier state only after successful send —
            // if handle_key fails, we don't want stale "pressed" state
            // affecting subsequent key event decisions.
            self.modifier_state.update(&key, true);
        }
        Ok(())
    }

    fn key_up(&mut self, key: Key) {
        // Intentionally asymmetric with key_down: update state BEFORE sending.
        // On release, we always mark as released even if the send fails below,
        // to avoid permanently stuck-modifier state in our tracker. The trade-off
        // (tracker says "released" while OS may still have it pressed) is acceptable
        // because such failures are rare and subsequent events will resynchronize.
        self.modifier_state.update(&key, false);

        if let Key::Layout(chr) = key {
            // ASCII characters: send keysym up if we also sent it on key_down
            let keysym = char_to_keysym(chr);
            if can_input_via_keysym(chr, keysym) {
                if let Err(e) = send_keysym(keysym, false, self.conn.clone(), &self.session) {
                    log::error!("Failed to send keysym up: {:?}", e);
                }
            }
            // Non-ASCII: already handled completely in key_down via clipboard paste,
            // no corresponding release needed (clipboard paste is an atomic operation)
        } else {
            if let Err(e) = handle_key(false, key, self.conn.clone(), &self.session) {
                log::error!("Failed to handle key up: {:?}", e);
            }
        }
    }

    fn key_click(&mut self, key: Key) {
        if let Key::Layout(chr) = key {
            let keysym = char_to_keysym(chr);
            // ASCII characters: use keysym
            if can_input_via_keysym(chr, keysym) {
                if let Err(e) = send_keysym(keysym, true, self.conn.clone(), &self.session) {
                    log::error!("Failed to send keysym down: {:?}", e);
                }
                if let Err(e) = send_keysym(keysym, false, self.conn.clone(), &self.session) {
                    log::error!("Failed to send keysym up: {:?}", e);
                }
            } else {
                // Non-ASCII: use clipboard
                input_text_via_clipboard(&chr.to_string(), self.conn.clone(), &self.session);
            }
        } else {
            if let Err(e) = handle_key(true, key.clone(), self.conn.clone(), &self.session) {
                log::error!("Failed to handle key down: {:?}", e);
            } else {
                // Only mark modifier as pressed if key-down was actually delivered
                self.modifier_state.update(&key, true);
            }
            // Always mark as released to avoid stuck-modifier state
            self.modifier_state.update(&key, false);
            if let Err(e) = handle_key(false, key, self.conn.clone(), &self.session) {
                log::error!("Failed to handle key up: {:?}", e);
            }
        }
    }
}
