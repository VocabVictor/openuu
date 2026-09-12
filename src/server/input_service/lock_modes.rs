use super::*;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(super) struct LockModesHandler {
    caps_lock_changed: bool,
    num_lock_changed: bool,
}

#[cfg(target_os = "macos")]
pub(super) struct LockModesHandler;

impl LockModesHandler {
    #[inline]
    pub(super) fn is_modifier_enabled(key_event: &KeyEvent, modifier: ControlKey) -> bool {
        key_event.modifiers.contains(&modifier.into())
    }

    #[inline]
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub(super) fn new_handler(key_event: &KeyEvent, _is_numpad_key: bool) -> Self {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            Self::new(key_event, _is_numpad_key)
        }
        #[cfg(target_os = "macos")]
        {
            Self::new(key_event)
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn sleep_to_ensure_locked(v: bool, k: enigo::Key, en: &mut Enigo) {
        if wayland_use_uinput() {
            // Sleep at most 500ms to ensure the lock state is applied.
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if en.get_key_state(k) == v {
                    break;
                }
            }
        } else if wayland_use_rdp_input() {
            // We can't call `en.get_key_state(k)` because there's no api for this.
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    pub(super) fn new(key_event: &KeyEvent, is_numpad_key: bool) -> Self {
        let mut en = ENIGO.lock().unwrap();
        let event_caps_enabled = Self::is_modifier_enabled(key_event, ControlKey::CapsLock);
        let local_caps_enabled = en.get_key_state(enigo::Key::CapsLock);
        let caps_lock_changed = event_caps_enabled != local_caps_enabled;
        if caps_lock_changed {
            en.key_click(enigo::Key::CapsLock);
            #[cfg(target_os = "linux")]
            Self::sleep_to_ensure_locked(event_caps_enabled, enigo::Key::CapsLock, &mut en);
        }

        let mut num_lock_changed = false;
        #[allow(unused)]
        let mut event_num_enabled = false;
        if is_numpad_key {
            let local_num_enabled = en.get_key_state(enigo::Key::NumLock);
            event_num_enabled = Self::is_modifier_enabled(key_event, ControlKey::NumLock);
            num_lock_changed = event_num_enabled != local_num_enabled;
        } else if is_legacy_mode(key_event) {
            #[cfg(target_os = "windows")]
            {
                num_lock_changed =
                    should_disable_numlock(key_event) && en.get_key_state(enigo::Key::NumLock);
            }
        }
        if num_lock_changed {
            en.key_click(enigo::Key::NumLock);
            #[cfg(target_os = "linux")]
            Self::sleep_to_ensure_locked(event_num_enabled, enigo::Key::NumLock, &mut en);
        }

        Self {
            caps_lock_changed,
            num_lock_changed,
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn new(key_event: &KeyEvent) -> Self {
        let event_caps_enabled = Self::is_modifier_enabled(key_event, ControlKey::CapsLock);
        // Do not use the following code to detect `local_caps_enabled`.
        // Because the state of get_key_state will not affect simulation of `VIRTUAL_INPUT_STATE` in this file.
        //
        // let local_caps_enabled = VirtualInput::get_key_state(
        //     CGEventSourceStateID::CombinedSessionState,
        //     rdev::kVK_CapsLock,
        // );
        let local_caps_enabled = unsafe {
            let _lock = VIRTUAL_INPUT_MTX.lock();
            VIRTUAL_INPUT_STATE
                .as_ref()
                .map_or(false, |input| input.capslock_down)
        };
        if event_caps_enabled && !local_caps_enabled {
            press_capslock();
        } else if !event_caps_enabled && local_caps_enabled {
            release_capslock();
        }

        Self {}
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Drop for LockModesHandler {
    fn drop(&mut self) {
        // Do not change led state if is Wayland uinput.
        // Because there must be a delay to ensure the lock state is applied on Wayland uinput,
        // which may affect the user experience.
        #[cfg(target_os = "linux")]
        if wayland_use_uinput() {
            return;
        }

        let mut en = ENIGO.lock().unwrap();
        if self.caps_lock_changed {
            en.key_click(enigo::Key::CapsLock);
        }
        if self.num_lock_changed {
            en.key_click(enigo::Key::NumLock);
        }
    }
}

#[inline]
#[cfg(target_os = "windows")]
pub(super) fn should_disable_numlock(evt: &KeyEvent) -> bool {
    // disable numlock if press home etc when numlock is on,
    // because we will get numpad value (7,8,9 etc) if not
    match (&evt.union, evt.mode.enum_value_or(KeyboardMode::Legacy)) {
        (Some(key_event::Union::ControlKey(ck)), KeyboardMode::Legacy) => {
            return NUMPAD_KEY_MAP.contains_key(&ck.value());
        }
        _ => {}
    }
    false
}
