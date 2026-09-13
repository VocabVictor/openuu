use super::*;

/// Send a keysym via RemoteDesktop portal.
pub(super) fn send_keysym(
    keysym: i32,
    down: bool,
    conn: Arc<SyncConnection>,
    session: &Path<'static>,
) -> ResultType<()> {
    let state: u32 = if down {
        PRESSED_DOWN_STATE
    } else {
        PRESSED_UP_STATE
    };
    let portal = get_portal(&conn);
    log::trace!(
        "send_keysym: calling notify_keyboard_keysym, keysym={:#x}, state={}",
        keysym,
        state
    );
    match remote_desktop_portal::notify_keyboard_keysym(
        &portal,
        session,
        HashMap::new(),
        keysym,
        state,
    ) {
        Ok(_) => {
            log::trace!("send_keysym: notify_keyboard_keysym succeeded");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

pub(super) fn get_raw_evdev_keycode(key: u16) -> i32 {
    // 8 is the offset between xkb and evdev
    let mut key = key as i32 - 8;
    // fix for right_meta key
    if key == 126 {
        key = 125;
    }
    key
}

pub(super) fn handle_key(
    down: bool,
    key: Key,
    conn: Arc<SyncConnection>,
    session: &Path<'static>,
) -> ResultType<()> {
    let state: u32 = if down {
        PRESSED_DOWN_STATE
    } else {
        PRESSED_UP_STATE
    };
    let portal = get_portal(&conn);
    match key {
        Key::Raw(key) => {
            let key = get_raw_evdev_keycode(key);
            remote_desktop_portal::notify_keyboard_keycode(
                &portal,
                &session,
                HashMap::new(),
                key,
                state,
            )?;
        }
        _ => {
            if let Ok((key, is_shift)) = map_key(&key) {
                let shift_keycode = evdev::Key::KEY_LEFTSHIFT.code() as i32;
                if down {
                    // Press: Shift down first, then key down
                    if is_shift {
                        if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                            &portal,
                            &session,
                            HashMap::new(),
                            shift_keycode,
                            state,
                        ) {
                            log::error!("handle_key: failed to press Shift: {:?}", e);
                            return Err(e.into());
                        }
                    }
                    if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                        &portal,
                        &session,
                        HashMap::new(),
                        key.code() as i32,
                        state,
                    ) {
                        log::error!("handle_key: failed to press key: {:?}", e);
                        // Best-effort: release Shift if it was pressed
                        if is_shift {
                            if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                                &portal,
                                &session,
                                HashMap::new(),
                                shift_keycode,
                                PRESSED_UP_STATE,
                            ) {
                                log::warn!(
                                    "handle_key: best-effort Shift release also failed: {:?}",
                                    e
                                );
                            }
                        }
                        return Err(e.into());
                    }
                } else {
                    // Release: key up first, then Shift up
                    if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                        &portal,
                        &session,
                        HashMap::new(),
                        key.code() as i32,
                        PRESSED_UP_STATE,
                    ) {
                        log::error!("handle_key: failed to release key: {:?}", e);
                        // Best-effort: still try to release Shift
                        if is_shift {
                            if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                                &portal,
                                &session,
                                HashMap::new(),
                                shift_keycode,
                                PRESSED_UP_STATE,
                            ) {
                                log::warn!(
                                    "handle_key: best-effort Shift release also failed: {:?}",
                                    e
                                );
                            }
                        }
                        return Err(e.into());
                    }
                    if is_shift {
                        if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                            &portal,
                            &session,
                            HashMap::new(),
                            shift_keycode,
                            PRESSED_UP_STATE,
                        ) {
                            log::error!("handle_key: failed to release Shift: {:?}", e);
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn handle_mouse(
    down: bool,
    button: MouseButton,
    conn: Arc<SyncConnection>,
    session: &Path<'static>,
) {
    let portal = get_portal(&conn);
    let but_key = match button {
        MouseButton::Left => EVDEV_MOUSE_LEFT,
        MouseButton::Right => EVDEV_MOUSE_RIGHT,
        MouseButton::Middle => EVDEV_MOUSE_MIDDLE,
        _ => {
            return;
        }
    };
    let state: u32 = if down {
        PRESSED_DOWN_STATE
    } else {
        PRESSED_UP_STATE
    };
    let _ = remote_desktop_portal::notify_pointer_button(
        &portal,
        &session,
        HashMap::new(),
        but_key,
        state,
    );
}
