use super::*;

pub fn is_left_up(evt: &MouseEvent) -> bool {
    let buttons = evt.mask >> 3;
    let evt_type = evt.mask & MOUSE_TYPE_MASK;
    buttons == MOUSE_BUTTON_LEFT && evt_type == MOUSE_TYPE_UP
}

#[cfg(windows)]
pub fn mouse_move_relative(x: i32, y: i32) {
    crate::platform::windows::try_change_desktop();
    let mut en = ENIGO.lock().unwrap();
    en.mouse_move_relative(x, y);
}

#[cfg(windows)]
pub(super) fn modifier_sleep() {
    // sleep for a while, this is only for keying in rdp in peer so far
    std::thread::sleep(std::time::Duration::from_nanos(1));
}

#[inline]
#[cfg(not(target_os = "macos"))]
pub(super) fn is_pressed(key: &Key, en: &mut Enigo) -> bool {
    get_modifier_state(key.clone(), en)
}

// Sleep for 8ms is enough in my tests, but we sleep 12ms to be safe.
// sleep 12ms In my test, the characters are already output in real time.
#[inline]
#[cfg(target_os = "macos")]
pub(super) fn key_sleep() {
    // https://www.reddit.com/r/rustdesk/comments/1kn1w5x/typing_lags_when_connecting_to_macos_clients/
    //
    // There's a strange bug when running by `launchctl load -w /Library/LaunchAgents/abc.plist`
    // `std::thread::sleep(Duration::from_millis(20));` may sleep 90ms or more.
    // Though `/Applications/RustDesk.app/Contents/MacOS/rustdesk --server` in terminal is ok.
    let now = Instant::now();
    while now.elapsed() < Duration::from_millis(12) {
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[inline]
pub(super) fn get_modifier_state(key: Key, en: &mut Enigo) -> bool {
    // https://github.com/rustdesk/rustdesk/issues/332
    // on Linux, if RightAlt is down, RightAlt status is false, Alt status is true
    // but on Windows, both are true
    let x = en.get_key_state(key.clone());
    match key {
        Key::Shift => x || en.get_key_state(Key::RightShift),
        Key::Control => x || en.get_key_state(Key::RightControl),
        Key::Alt => x || en.get_key_state(Key::RightAlt),
        Key::Meta => x || en.get_key_state(Key::RWin),
        Key::RightShift => x || en.get_key_state(Key::Shift),
        Key::RightControl => x || en.get_key_state(Key::Control),
        Key::RightAlt => x || en.get_key_state(Key::Alt),
        Key::RWin => x || en.get_key_state(Key::Meta),
        _ => x,
    }
}

#[allow(unreachable_code)]
pub fn handle_mouse(
    evt: &MouseEvent,
    conn: i32,
    username: String,
    argb: u32,
    simulate: bool,
    show_cursor: bool,
) {
    #[cfg(target_os = "macos")]
    {
        // having GUI (--server has tray, it is GUI too), run main GUI thread, otherwise crash
        let evt = evt.clone();
        QUEUE.exec_async(move || handle_mouse_(&evt, conn, username, argb, simulate, show_cursor));
        return;
    }
    #[cfg(windows)]
    crate::portable_service::client::handle_mouse(evt, conn, username, argb, simulate, show_cursor);
    #[cfg(not(windows))]
    handle_mouse_(evt, conn, username, argb, simulate, show_cursor);
}

// to-do: merge handle_mouse and handle_pointer
#[allow(unreachable_code)]
pub fn handle_pointer(evt: &PointerDeviceEvent, conn: i32) {
    #[cfg(target_os = "macos")]
    {
        // having GUI, run main GUI thread, otherwise crash
        let evt = evt.clone();
        QUEUE.exec_async(move || handle_pointer_(&evt, conn));
        return;
    }
    #[cfg(windows)]
    crate::portable_service::client::handle_pointer(evt, conn);
    #[cfg(not(windows))]
    handle_pointer_(evt, conn);
}

// Update time to avoid send cursor position event to the peer.
// See `run_pos` --> `set_cursor_position` --> `exclude`
#[inline]
pub fn update_latest_input_cursor_time(conn: i32) {
    let mut lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
    lock.conn = conn;
    lock.time = get_time();
}

#[inline]
pub(super) fn get_last_input_cursor_pos() -> (i32, i32) {
    let lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
    (lock.x, lock.y)
}

// check if mouse is moved by the controlled side user to make controlled side has higher mouse priority than remote.
pub(super) fn active_mouse_(_conn: i32) -> bool {
    true
    /* this method is buggy (not working on macOS, making fast moving mouse event discarded here) and added latency (this is blocking way, must do in async way), so we disable it for now
    // out of time protection
    if LATEST_SYS_CURSOR_POS
        .lock()
        .unwrap()
        .0
        .map(|t| t.elapsed() > MOUSE_MOVE_PROTECTION_TIMEOUT)
        .unwrap_or(true)
    {
        return true;
    }

    // last conn input may be protected
    if LATEST_PEER_INPUT_CURSOR.lock().unwrap().conn != conn {
        return false;
    }

    let in_active_dist = |a: i32, b: i32| -> bool { (a - b).abs() < MOUSE_ACTIVE_DISTANCE };

    // Check if input is in valid range
    match crate::get_cursor_pos() {
        Some((x, y)) => {
            let (last_in_x, last_in_y) = get_last_input_cursor_pos();
            let mut can_active = in_active_dist(last_in_x, x) && in_active_dist(last_in_y, y);
            // The cursor may not have been moved to last input position if system is busy now.
            // While this is not a common case, we check it again after some time later.
            if !can_active {
                // 100 micros may be enough for system to move cursor.
                // Mouse inputs on macOS are asynchronous. 1. Put in a queue to process in main thread. 2. Send event async.
                // More reties are needed on macOS.
                #[cfg(not(target_os = "macos"))]
                let retries = 10;
                #[cfg(target_os = "macos")]
                let retries = 100;
                #[cfg(not(target_os = "macos"))]
                let sleep_interval: u64 = 10;
                #[cfg(target_os = "macos")]
                let sleep_interval: u64 = 30;
                for _retry in 0..retries {
                    std::thread::sleep(std::time::Duration::from_micros(sleep_interval));
                    // Sleep here can also somehow suppress delay accumulation.
                    if let Some((x2, y2)) = crate::get_cursor_pos() {
                        let (last_in_x, last_in_y) = get_last_input_cursor_pos();
                        can_active = in_active_dist(last_in_x, x2) && in_active_dist(last_in_y, y2);
                        if can_active {
                            break;
                        }
                    }
                }
            }
            if !can_active {
                let mut lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
                lock.x = INVALID_CURSOR_POS / 2;
                lock.y = INVALID_CURSOR_POS / 2;
            }
            can_active
        }
        None => true,
    }
    */
}

pub fn handle_pointer_(evt: &PointerDeviceEvent, conn: i32) {
    if !active_mouse_(conn) {
        return;
    }

    if EXITING.load(Ordering::SeqCst) {
        return;
    }

    match &evt.union {
        Some(TouchEvent(evt)) => match &evt.union {
            Some(ScaleUpdate(_scale_evt)) => {
                #[cfg(target_os = "windows")]
                handle_scale(_scale_evt.scale);
            }
            _ => {}
        },
        _ => {}
    }
}

pub fn handle_mouse_(
    evt: &MouseEvent,
    conn: i32,
    _username: String,
    _argb: u32,
    simulate: bool,
    _show_cursor: bool,
) {
    if simulate {
        handle_mouse_simulation_(evt, conn);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let evt_type = evt.mask & MOUSE_TYPE_MASK;
        // Relative (delta) mouse events do not include absolute coordinates, so
        // whiteboard/cursor rendering must be disabled during relative mode to prevent
        // incorrect cursor/whiteboard updates. We check both is_relative_mouse_active(conn)
        // (connection already in relative mode from prior events) and evt_type (current
        // event is relative) to guard against the first relative event before the flag is set.
        if _show_cursor && !is_relative_mouse_active(conn) && evt_type != MOUSE_TYPE_MOVE_RELATIVE {
            handle_mouse_show_cursor_(evt, conn, _username, _argb);
        }
    }
}
