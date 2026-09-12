use super::*;

pub fn is_enter(evt: &KeyEvent) -> bool {
    if let Some(key_event::Union::ControlKey(ck)) = evt.union {
        if ck.value() == ControlKey::Return.value() || ck.value() == ControlKey::NumpadEnter.value()
        {
            return true;
        }
    }
    return false;
}

pub async fn lock_screen() {
    cfg_if::cfg_if! {
    if #[cfg(target_os = "linux")] {
        // xdg_screensaver lock not work on Linux from our service somehow
        // loginctl lock-session also not work, they both work run rustdesk from cmd
        std::thread::spawn(|| {
            let mut key_event = KeyEvent::new();

            key_event.set_chr('l' as _);
            key_event.modifiers.push(ControlKey::Meta.into());
            key_event.mode = KeyboardMode::Legacy.into();

            key_event.down = true;
            handle_key(&key_event);

            key_event.down = false;
            handle_key(&key_event);
        });
    } else if #[cfg(target_os = "macos")] {
        // CGSession -suspend not real lock screen, it is user switch
        std::thread::spawn(|| {
            let mut key_event = KeyEvent::new();

            key_event.set_chr('q' as _);
            key_event.modifiers.push(ControlKey::Meta.into());
            key_event.modifiers.push(ControlKey::Control.into());
            key_event.mode = KeyboardMode::Legacy.into();

            key_event.down = true;
            handle_key(&key_event);
            key_event.down = false;
            handle_key(&key_event);
        });
    } else {
    crate::platform::lock_screen();
    }
    }
}

#[inline]
#[cfg(target_os = "linux")]
pub fn handle_key(evt: &KeyEvent) {
    handle_key_(evt);
}

#[inline]
#[cfg(target_os = "windows")]
pub fn handle_key(evt: &KeyEvent) {
    crate::portable_service::client::handle_key(evt);
}

#[inline]
#[cfg(target_os = "macos")]
pub fn handle_key(evt: &KeyEvent) {
    // having GUI, run main GUI thread, otherwise crash
    let evt = evt.clone();
    QUEUE.exec_async(move || handle_key_(&evt));
    // Key sleep is required for macOS.
    // If we don't sleep, the key press/release events may not take effect.
    //
    // For example, the controlled side osx `12.7.6` or `15.1.1`
    // If we input characters quickly and continuously, and press or release "Shift" for a short period of time,
    // it is possible that after releasing "Shift", the controlled side will still print uppercase characters.
    // Though it is not very easy to reproduce.
    key_sleep();
}

#[cfg(target_os = "macos")]
#[inline]
pub(super) fn reset_input() {
    unsafe {
        let _lock = VIRTUAL_INPUT_MTX.lock();
        VIRTUAL_INPUT_STATE = VirtualInputState::new();
    }
}

#[cfg(target_os = "macos")]
pub fn reset_input_ondisconn() {
    QUEUE.exec_async(reset_input);
}

pub(super) fn sim_rdev_rawkey_position(code: KeyCode, keydown: bool) {
    #[cfg(target_os = "windows")]
    let rawkey = RawKey::ScanCode(code);
    #[cfg(target_os = "linux")]
    let rawkey = RawKey::LinuxXorgKeycode(code);
    // // to-do: test android
    // #[cfg(target_os = "android")]
    // let rawkey = RawKey::LinuxConsoleKeycode(code);
    #[cfg(target_os = "macos")]
    let rawkey = RawKey::MacVirtualKeycode(code);

    // map mode(1): Send keycode according to the peer platform.
    record_pressed_key(KeysDown::RdevKey(rawkey), keydown);

    let event_type = if keydown {
        EventType::KeyPress(RdevKey::RawKey(rawkey))
    } else {
        EventType::KeyRelease(RdevKey::RawKey(rawkey))
    };
    simulate_(&event_type);
}

#[cfg(target_os = "windows")]
pub(super) fn sim_rdev_rawkey_virtual(code: u32, keydown: bool) {
    let rawkey = RawKey::WinVirtualKeycode(code);
    record_pressed_key(KeysDown::RdevKey(rawkey), keydown);
    let event_type = if keydown {
        EventType::KeyPress(RdevKey::RawKey(rawkey))
    } else {
        EventType::KeyRelease(RdevKey::RawKey(rawkey))
    };
    simulate_(&event_type);
}

#[inline]
#[cfg(target_os = "macos")]
pub(super) fn simulate_(event_type: &EventType) {
    unsafe {
        let _lock = VIRTUAL_INPUT_MTX.lock();
        if let Some(input) = VIRTUAL_INPUT_STATE.as_ref() {
            let _ = input.simulate(&event_type);
        }
    }
}

#[inline]
#[cfg(target_os = "macos")]
pub(super) fn press_capslock() {
    let caps_key = RdevKey::RawKey(rdev::RawKey::MacVirtualKeycode(rdev::kVK_CapsLock));
    unsafe {
        let _lock = VIRTUAL_INPUT_MTX.lock();
        if let Some(input) = VIRTUAL_INPUT_STATE.as_mut() {
            if input.simulate(&EventType::KeyPress(caps_key)).is_ok() {
                input.capslock_down = true;
                key_sleep();
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[inline]
pub(super) fn release_capslock() {
    let caps_key = RdevKey::RawKey(rdev::RawKey::MacVirtualKeycode(rdev::kVK_CapsLock));
    unsafe {
        let _lock = VIRTUAL_INPUT_MTX.lock();
        if let Some(input) = VIRTUAL_INPUT_STATE.as_mut() {
            if input.simulate(&EventType::KeyRelease(caps_key)).is_ok() {
                input.capslock_down = false;
                key_sleep();
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[inline]
pub(super) fn simulate_(event_type: &EventType) {
    match rdev::simulate(&event_type) {
        Ok(()) => (),
        Err(_simulate_error) => {
            log::error!("Could not send {:?}", &event_type);
        }
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn is_legacy_mode(evt: &KeyEvent) -> bool {
    evt.mode.enum_value_or(KeyboardMode::Legacy) == KeyboardMode::Legacy
}

pub fn handle_key_(evt: &KeyEvent) {
    if EXITING.load(Ordering::SeqCst) {
        return;
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let mut _lock_mode_handler = None;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    match &evt.union {
        Some(key_event::Union::Unicode(..)) | Some(key_event::Union::Seq(..)) => {
            _lock_mode_handler = Some(LockModesHandler::new_handler(&evt, false));
        }
        Some(key_event::Union::ControlKey(ck)) => {
            let key = ck.enum_value_or(ControlKey::Unknown);
            if !skip_led_sync_control_key(&key) {
                #[cfg(target_os = "macos")]
                let is_numpad_key = false;
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                let is_numpad_key = is_numpad_control_key(&key);
                _lock_mode_handler = Some(LockModesHandler::new_handler(&evt, is_numpad_key));
            }
        }
        Some(key_event::Union::Chr(code)) => {
            if is_legacy_mode(&evt) {
                _lock_mode_handler = Some(LockModesHandler::new_handler(evt, false));
            } else {
                let key = crate::keyboard::keycode_to_rdev_key(*code);
                if !skip_led_sync_rdev_key(&key) {
                    #[cfg(target_os = "macos")]
                    let is_numpad_key = false;
                    #[cfg(any(target_os = "windows", target_os = "linux"))]
                    let is_numpad_key = crate::keyboard::is_numpad_rdev_key(&key);
                    _lock_mode_handler = Some(LockModesHandler::new_handler(evt, is_numpad_key));
                }
            }
        }
        _ => {}
    }

    match evt.mode.enum_value() {
        Ok(KeyboardMode::Map) => {
            #[cfg(target_os = "macos")]
            set_last_legacy_mode(false);
            map_keyboard_mode(evt);
        }
        Ok(KeyboardMode::Translate) => {
            #[cfg(target_os = "macos")]
            set_last_legacy_mode(false);
            translate_keyboard_mode(evt);
        }
        _ => {
            // All key down events are started from here,
            // so we can reset the flag of last legacy mode here.
            #[cfg(target_os = "macos")]
            set_last_legacy_mode(true);
            legacy_keyboard_mode(evt);
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn lock_screen_2() {
    lock_screen().await;
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
pub(super) async fn send_sas() -> ResultType<()> {
    if crate::platform::is_physical_console_session().unwrap_or(true) {
        let mut stream = crate::ipc::connect(1000, crate::POSTFIX_SERVICE).await?;
        timeout(1000, stream.send(&crate::ipc::Data::SAS)).await??;
    } else {
        crate::platform::send_sas();
    };
    Ok(())
}
