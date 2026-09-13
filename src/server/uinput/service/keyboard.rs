use super::*;

pub(super) fn create_uinput_keyboard() -> ResultType<VirtualDevice> {
    // TODO: ensure keys here
    let mut keys = AttributeSet::<evdev::Key>::new();
    for i in evdev::Key::KEY_ESC.code()..(evdev::Key::BTN_TRIGGER_HAPPY40.code() + 1) {
        let key = evdev::Key::new(i);
        if !format!("{:?}", &key).contains("unknown key") {
            keys.insert(key);
        }
    }
    let mut leds = AttributeSet::<evdev::LedType>::new();
    leds.insert(evdev::LedType::LED_NUML);
    leds.insert(evdev::LedType::LED_CAPSL);
    leds.insert(evdev::LedType::LED_SCROLLL);
    let mut miscs = AttributeSet::<evdev::MiscType>::new();
    miscs.insert(evdev::MiscType::MSC_SCAN);
    let keyboard = VirtualDeviceBuilder::new()?
        .name("RustDesk UInput Keyboard")
        .with_keys(&keys)?
        .with_leds(&leds)?
        .with_miscs(&miscs)?
        .build()?;
    Ok(keyboard)
}

pub fn map_key(key: &enigo::Key) -> ResultType<(evdev::Key, bool)> {
    if let Some(k) = KEY_MAP.get(&key) {
        log::trace!("mapkey matched in KEY_MAP, evdev={:?}", &k);
        return Ok((k.clone(), false));
    } else {
        match key {
            enigo::Key::Layout(c) => {
                if let Some((k, is_shift)) = KEY_MAP_LAYOUT.get(&c) {
                    log::trace!("mapkey Layout matched, evdev={:?}", k);
                    return Ok((k.clone(), is_shift.clone()));
                }
            }
            // enigo::Key::Raw(c) => {
            //     let k = evdev::Key::new(c);
            //     if !format!("{:?}", &k).contains("unknown key") {
            //         return Ok(k.clone());
            //     }
            // }
            _ => {}
        }
    }
    bail!("Failed to map key {:?}", &key);
}

pub(super) async fn ipc_send_data(stream: &mut Connection, data: &Data) {
    allow_err!(stream.send(data).await);
}

pub(super) async fn handle_keyboard(
    stream: &mut Connection,
    keyboard: &mut VirtualDevice,
    data: &DataKeyboard,
) {
    let data_desc = match data {
        DataKeyboard::Sequence(seq) => format!("Sequence(len={})", seq.len()),
        DataKeyboard::KeyDown(Key::Layout(_))
        | DataKeyboard::KeyUp(Key::Layout(_))
        | DataKeyboard::KeyClick(Key::Layout(_)) => "Layout(<redacted>)".to_string(),
        _ => format!("{:?}", data),
    };
    log::trace!("handle_keyboard received: {}", data_desc);
    match data {
        DataKeyboard::Sequence(seq) => {
            // Normally handled by --server process (input_text_via_clipboard_server).
            // Fallback: input_text_wayland handles ASCII via keysym/uinput;
            // non-ASCII will be skipped (no clipboard access in --service process).
            if !seq.is_empty() {
                input_text_wayland(seq, keyboard);
            }
        }
        DataKeyboard::KeyDown(enigo::Key::Raw(code)) => {
            if *code < 8 {
                log::error!(
                    "Invalid Raw keycode {} (must be >= 8 due to XKB offset), skipping",
                    code
                );
            } else {
                let down_event = InputEvent::new(EventType::KEY, *code - 8, 1);
                allow_err!(keyboard.emit(&[down_event]));
            }
        }
        DataKeyboard::KeyUp(enigo::Key::Raw(code)) => {
            if *code < 8 {
                log::error!(
                    "Invalid Raw keycode {} (must be >= 8 due to XKB offset), skipping",
                    code
                );
            } else {
                let up_event = InputEvent::new(EventType::KEY, *code - 8, 0);
                allow_err!(keyboard.emit(&[up_event]));
            }
        }
        DataKeyboard::KeyDown(key) => {
            if let Key::Layout(chr) = key {
                input_char_wayland_key_event(*chr, true, keyboard);
            } else {
                if let Ok((k, _is_shift)) = map_key(key) {
                    let down_event = InputEvent::new(EventType::KEY, k.code(), 1);
                    allow_err!(keyboard.emit(&[down_event]));
                }
            }
        }
        DataKeyboard::KeyUp(key) => {
            if let Key::Layout(chr) = key {
                input_char_wayland_key_event(*chr, false, keyboard);
            } else {
                if let Ok((k, _)) = map_key(key) {
                    let up_event = InputEvent::new(EventType::KEY, k.code(), 0);
                    allow_err!(keyboard.emit(&[up_event]));
                }
            }
        }
        DataKeyboard::KeyClick(key) => {
            if let Key::Layout(chr) = key {
                input_text_wayland(&chr.to_string(), keyboard);
            } else {
                if let Ok((k, _is_shift)) = map_key(key) {
                    let down_event = InputEvent::new(EventType::KEY, k.code(), 1);
                    let up_event = InputEvent::new(EventType::KEY, k.code(), 0);
                    allow_err!(keyboard.emit(&[down_event, up_event]));
                }
            }
        }
        DataKeyboard::GetKeyState(key) => {
            let key_state = if enigo::Key::CapsLock == *key {
                match keyboard.get_led_state() {
                    Ok(leds) => leds.contains(evdev::LedType::LED_CAPSL),
                    Err(_e) => {
                        // log::debug!("Failed to get led state {}", &_e);
                        false
                    }
                }
            } else if enigo::Key::NumLock == *key {
                match keyboard.get_led_state() {
                    Ok(leds) => leds.contains(evdev::LedType::LED_NUML),
                    Err(_e) => {
                        // log::debug!("Failed to get led state {}", &_e);
                        false
                    }
                }
            } else {
                match keyboard.get_key_state() {
                    Ok(keys) => match key {
                        enigo::Key::Shift => {
                            keys.contains(evdev::Key::KEY_LEFTSHIFT)
                                || keys.contains(evdev::Key::KEY_RIGHTSHIFT)
                        }
                        enigo::Key::Control => {
                            keys.contains(evdev::Key::KEY_LEFTCTRL)
                                || keys.contains(evdev::Key::KEY_RIGHTCTRL)
                        }
                        enigo::Key::Alt => {
                            keys.contains(evdev::Key::KEY_LEFTALT)
                                || keys.contains(evdev::Key::KEY_RIGHTALT)
                        }
                        enigo::Key::Meta => {
                            keys.contains(evdev::Key::KEY_LEFTMETA)
                                || keys.contains(evdev::Key::KEY_RIGHTMETA)
                        }
                        _ => false,
                    },
                    Err(_e) => {
                        // log::debug!("Failed to get key state: {}", &_e);
                        false
                    }
                }
            };
            ipc_send_data(
                stream,
                &Data::KeyboardResponse(ipc::DataKeyboardResponse::GetKeyState(key_state)),
            )
            .await;
        }
    }
}
