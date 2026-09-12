use super::*;

pub fn handle_mouse_simulation_(evt: &MouseEvent, conn: i32) {
    if !active_mouse_(conn) {
        return;
    }

    if EXITING.load(Ordering::SeqCst) {
        return;
    }

    #[cfg(windows)]
    crate::platform::windows::try_change_desktop();
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & MOUSE_TYPE_MASK;
    let mut en = ENIGO.lock().unwrap();
    #[cfg(target_os = "macos")]
    en.set_ignore_flags(enigo_ignore_flags());
    #[cfg(not(target_os = "macos"))]
    let mut to_release = Vec::new();
    if evt_type == MOUSE_TYPE_DOWN {
        fix_modifiers(&evt.modifiers[..], &mut en, 0);
        #[cfg(target_os = "macos")]
        en.reset_flag();
        for ref ck in evt.modifiers.iter() {
            if let Some(key) = KEY_MAP.get(&ck.value()) {
                #[cfg(target_os = "macos")]
                en.add_flag(key);
                #[cfg(not(target_os = "macos"))]
                if key != &Key::CapsLock && key != &Key::NumLock {
                    if !get_modifier_state(key.clone(), &mut en) {
                        en.key_down(key.clone()).ok();
                        #[cfg(windows)]
                        modifier_sleep();
                        to_release.push(key);
                    }
                }
            }
        }
    }
    match evt_type {
        MOUSE_TYPE_MOVE => {
            // Switching back to absolute movement implicitly disables relative mouse mode.
            set_relative_mouse_active(conn, false);
            // On Wayland with uinput, the client sends coordinates in the layout it was
            // told at session init. If the compositor has since moved a monitor, correct
            // them onto the current layout. https://github.com/rustdesk/rustdesk/issues/15601
            #[cfg(target_os = "linux")]
            let (mx, my) = if wayland_use_uinput() {
                super::display_service::remap_wayland_uinput_coord(evt.x, evt.y)
            } else {
                (evt.x, evt.y)
            };
            #[cfg(not(target_os = "linux"))]
            let (mx, my) = (evt.x, evt.y);
            en.mouse_move_to(mx, my);
            *LATEST_PEER_INPUT_CURSOR.lock().unwrap() = Input {
                conn,
                time: get_time(),
                x: mx,
                y: my,
            };
        }
        // MOUSE_TYPE_MOVE_RELATIVE: Relative mouse movement for gaming/3D applications.
        // Each client independently decides whether to use relative mode.
        // Multiple clients can mix absolute and relative movements without conflict,
        // as the server simply applies the delta to the current cursor position.
        MOUSE_TYPE_MOVE_RELATIVE => {
            set_relative_mouse_active(conn, true);
            // Clamp delta to prevent extreme/malicious values from reaching OS APIs.
            // This matches the Flutter client's kMaxRelativeMouseDelta constant.
            const MAX_RELATIVE_MOUSE_DELTA: i32 = 10000;
            let dx = evt
                .x
                .clamp(-MAX_RELATIVE_MOUSE_DELTA, MAX_RELATIVE_MOUSE_DELTA);
            let dy = evt
                .y
                .clamp(-MAX_RELATIVE_MOUSE_DELTA, MAX_RELATIVE_MOUSE_DELTA);
            en.mouse_move_relative(dx, dy);
            // Get actual cursor position after relative movement for tracking
            if let Some((x, y)) = crate::get_cursor_pos() {
                *LATEST_PEER_INPUT_CURSOR.lock().unwrap() = Input {
                    conn,
                    time: get_time(),
                    x,
                    y,
                };
            }
        }
        MOUSE_TYPE_DOWN => match buttons {
            MOUSE_BUTTON_LEFT => {
                allow_err!(en.mouse_down(MouseButton::Left));
            }
            MOUSE_BUTTON_RIGHT => {
                allow_err!(en.mouse_down(MouseButton::Right));
            }
            MOUSE_BUTTON_WHEEL => {
                allow_err!(en.mouse_down(MouseButton::Middle));
            }
            MOUSE_BUTTON_BACK => {
                allow_err!(en.mouse_down(MouseButton::Back));
            }
            MOUSE_BUTTON_FORWARD => {
                allow_err!(en.mouse_down(MouseButton::Forward));
            }
            _ => {}
        },
        MOUSE_TYPE_UP => match buttons {
            MOUSE_BUTTON_LEFT => {
                en.mouse_up(MouseButton::Left);
            }
            MOUSE_BUTTON_RIGHT => {
                en.mouse_up(MouseButton::Right);
            }
            MOUSE_BUTTON_WHEEL => {
                en.mouse_up(MouseButton::Middle);
            }
            MOUSE_BUTTON_BACK => {
                en.mouse_up(MouseButton::Back);
            }
            MOUSE_BUTTON_FORWARD => {
                en.mouse_up(MouseButton::Forward);
            }
            _ => {}
        },
        MOUSE_TYPE_WHEEL | MOUSE_TYPE_TRACKPAD => {
            #[allow(unused_mut)]
            let mut x = -evt.x;
            #[allow(unused_mut)]
            let mut y = evt.y;
            #[cfg(not(windows))]
            {
                y = -y;
            }

            #[cfg(any(target_os = "macos", target_os = "windows"))]
            let is_track_pad = evt_type == MOUSE_TYPE_TRACKPAD;

            #[cfg(target_os = "macos")]
            {
                // TODO: support track pad on win.

                // fix shift + scroll(down/up)
                if !is_track_pad
                    && evt
                        .modifiers
                        .contains(&EnumOrUnknown::new(ControlKey::Shift))
                {
                    x = y;
                    y = 0;
                }

                if x != 0 {
                    en.mouse_scroll_x(x, is_track_pad);
                }
                if y != 0 {
                    en.mouse_scroll_y(y, is_track_pad);
                }
            }

            #[cfg(windows)]
            if !is_track_pad {
                x *= WHEEL_DELTA as i32;
                y *= WHEEL_DELTA as i32;
            }

            #[cfg(not(target_os = "macos"))]
            {
                if y != 0 {
                    en.mouse_scroll_y(y);
                }
                if x != 0 {
                    en.mouse_scroll_x(x);
                }
            }
        }
        _ => {}
    }
    #[cfg(not(target_os = "macos"))]
    for key in to_release {
        en.key_up(key.clone());
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn handle_mouse_show_cursor_(evt: &MouseEvent, conn: i32, username: String, argb: u32) {
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & MOUSE_TYPE_MASK;
    match evt_type {
        MOUSE_TYPE_MOVE => {
            whiteboard::update_whiteboard(
                whiteboard::get_key_cursor(conn),
                whiteboard::CustomEvent::Cursor(whiteboard::Cursor {
                    x: evt.x as _,
                    y: evt.y as _,
                    argb,
                    btns: 0,
                    text: username,
                }),
            );
        }
        MOUSE_TYPE_UP => {
            if buttons == MOUSE_BUTTON_LEFT {
                // Some clients intentionally send button events without coordinates.
                // Fall back to the last known cursor position to avoid jumping to (0, 0).
                // TODO(protocol): (0, 0) is a valid screen coordinate. Consider using a dedicated
                // sentinel value (e.g. INVALID_CURSOR_POS) or a protocol-level flag to distinguish
                // "coordinates not provided" from "coordinates are (0, 0)". Impact is minor since
                // this only affects whiteboard rendering and clicking exactly at (0, 0) is rare.
                let (x, y) = if evt.x == 0 && evt.y == 0 {
                    get_last_input_cursor_pos()
                } else {
                    (evt.x, evt.y)
                };
                whiteboard::update_whiteboard(
                    whiteboard::get_key_cursor(conn),
                    whiteboard::CustomEvent::Cursor(whiteboard::Cursor {
                        x: x as _,
                        y: y as _,
                        argb,
                        btns: buttons,
                        text: username,
                    }),
                );
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "windows")]
pub(super) fn handle_scale(scale: i32) {
    let mut en = ENIGO.lock().unwrap();
    if scale == 0 {
        en.key_up(Key::Control);
    } else {
        if en.key_down(Key::Control).is_ok() {
            en.mouse_scroll_y(scale);
        }
    }
}
