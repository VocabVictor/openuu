use super::*;

pub fn fix_key_down_timeout_loop() {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(10_000));
        fix_key_down_timeout(false);
    });
    if let Err(err) = ctrlc::set_handler(move || {
        fix_key_down_timeout_at_exit();
        std::process::exit(0); // will call atexit on posix, but not on Windows
    }) {
        log::error!("Failed to set Ctrl-C handler: {}", err);
    }
}

pub fn fix_key_down_timeout_at_exit() {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }
    EXITING.store(true, Ordering::SeqCst);
    fix_key_down_timeout(true);
    log::info!("fix_key_down_timeout_at_exit");
}

#[inline]
#[cfg(target_os = "linux")]
pub fn clear_remapped_keycode() {
    ENIGO.lock().unwrap().tfc_clear_remapped();
}

#[inline]
pub(super) fn record_key_is_control_key(record_key: u64) -> bool {
    record_key < KEY_CHAR_START
}

#[inline]
pub(super) fn record_key_is_chr(record_key: u64) -> bool {
    record_key >= KEY_CHAR_START
}

#[inline]
pub(super) fn record_key_to_key(record_key: u64) -> Option<Key> {
    if record_key_is_control_key(record_key) {
        control_key_value_to_key(record_key as _)
    } else if record_key_is_chr(record_key) {
        let chr: u32 = (record_key - KEY_CHAR_START) as _;
        Some(char_value_to_key(chr))
    } else {
        None
    }
}

pub fn release_device_modifiers() {
    let mut en = ENIGO.lock().unwrap();
    for modifier in [
        Key::Shift,
        Key::Control,
        Key::Alt,
        Key::Meta,
        Key::RightShift,
        Key::RightControl,
        Key::RightAlt,
        Key::RWin,
    ] {
        if get_modifier_state(modifier, &mut en) {
            en.key_up(modifier);
        }
    }
}

#[inline]
pub(super) fn release_record_key(record_key: KeysDown) {
    let func = move || match record_key {
        KeysDown::RdevKey(raw_key) => {
            simulate_(&EventType::KeyRelease(RdevKey::RawKey(raw_key)));
        }
        KeysDown::EnigoKey(key) => {
            if let Some(key) = record_key_to_key(key) {
                ENIGO.lock().unwrap().key_up(key);
                log::debug!("Fixed {:?} timeout", key);
            }
        }
    };

    #[cfg(target_os = "macos")]
    QUEUE.exec_async(func);
    #[cfg(not(target_os = "macos"))]
    func();
}

pub(super) fn fix_key_down_timeout(force: bool) {
    let key_down = KEYS_DOWN.lock().unwrap();
    if key_down.is_empty() {
        return;
    }
    let cloned = (*key_down).clone();
    drop(key_down);

    for (record_key, time) in cloned.into_iter() {
        if force || time.elapsed().as_millis() >= 360_000 {
            record_pressed_key(record_key, false);
            release_record_key(record_key);
        }
    }
}

// e.g. current state of ctrl is down, but ctrl not in modifier, we should change ctrl to up, to make modifier state sync between remote and local
#[inline]
pub(super) fn fix_modifier(
    modifiers: &[EnumOrUnknown<ControlKey>],
    key0: ControlKey,
    key1: Key,
    en: &mut Enigo,
) {
    if get_modifier_state(key1, en) && !modifiers.contains(&EnumOrUnknown::new(key0)) {
        #[cfg(windows)]
        if key0 == ControlKey::Control && get_modifier_state(Key::Alt, en) {
            // AltGr case
            return;
        }
        en.key_up(key1);
        log::debug!("Fixed {:?}", key1);
    }
}

pub(super) fn fix_modifiers(modifiers: &[EnumOrUnknown<ControlKey>], en: &mut Enigo, ck: i32) {
    if ck != ControlKey::Shift.value() {
        fix_modifier(modifiers, ControlKey::Shift, Key::Shift, en);
    }
    if ck != ControlKey::RShift.value() {
        fix_modifier(modifiers, ControlKey::Shift, Key::RightShift, en);
    }
    if ck != ControlKey::Alt.value() {
        fix_modifier(modifiers, ControlKey::Alt, Key::Alt, en);
    }
    if ck != ControlKey::RAlt.value() {
        fix_modifier(modifiers, ControlKey::Alt, Key::RightAlt, en);
    }
    if ck != ControlKey::Control.value() {
        fix_modifier(modifiers, ControlKey::Control, Key::Control, en);
    }
    if ck != ControlKey::RControl.value() {
        fix_modifier(modifiers, ControlKey::Control, Key::RightControl, en);
    }
    if ck != ControlKey::Meta.value() {
        fix_modifier(modifiers, ControlKey::Meta, Key::Meta, en);
    }
    if ck != ControlKey::RWin.value() {
        fix_modifier(modifiers, ControlKey::Meta, Key::RWin, en);
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn release_keys(en: &mut Enigo, to_release: &Vec<Key>) {
    for key in to_release {
        en.key_up(key.clone());
    }
}

pub(super) fn record_pressed_key(record_key: KeysDown, down: bool) {
    let mut key_down = KEYS_DOWN.lock().unwrap();
    if down {
        key_down.insert(record_key, Instant::now());
    } else {
        key_down.remove(&record_key);
    }
}

pub(super) fn is_function_key(ck: &EnumOrUnknown<ControlKey>) -> bool {
    let mut res = false;
    if ck.value() == ControlKey::CtrlAltDel.value() {
        // have to spawn new thread because send_sas is tokio_main, the caller can not be tokio_main.
        #[cfg(windows)]
        std::thread::spawn(|| {
            allow_err!(send_sas());
        });
        res = true;
    } else if ck.value() == ControlKey::LockScreen.value() {
        std::thread::spawn(|| {
            lock_screen_2();
        });
        res = true;
    }
    return res;
}
