use super::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub(super) enum KeysDown {
    RdevKey(RawKey),
    EnigoKey(u64),
}

lazy_static::lazy_static! {
    pub(super) static ref ENIGO: Arc<Mutex<Enigo>> = {
        Arc::new(Mutex::new(Enigo::new()))
    };
    pub(super) static ref KEYS_DOWN: Arc<Mutex<HashMap<KeysDown, Instant>>> = Default::default();
    pub(super) static ref LATEST_PEER_INPUT_CURSOR: Arc<Mutex<Input>> = Default::default();
    pub(super) static ref LATEST_SYS_CURSOR_POS: Arc<Mutex<(Option<Instant>, (i32, i32))>> = Arc::new(Mutex::new((None, (INVALID_CURSOR_POS, INVALID_CURSOR_POS))));
    // Track connections that are currently using relative mouse movement.
    // Used to disable whiteboard/cursor display for all events while in relative mode.
    pub(super) static ref RELATIVE_MOUSE_CONNS: Arc<Mutex<std::collections::HashSet<i32>>> = Default::default();
}

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    pub(super) static ref WAYLAND_CLIPBOARD_INPUT_RECORDS: Arc<Mutex<Vec<(Instant, String)>>> =
        Default::default();
}

#[inline]
pub(super) fn set_relative_mouse_active(conn: i32, active: bool) {
    let mut lock = RELATIVE_MOUSE_CONNS.lock().unwrap();
    if active {
        lock.insert(conn);
    } else {
        lock.remove(&conn);
    }
}

#[inline]
pub(super) fn is_relative_mouse_active(conn: i32) -> bool {
    RELATIVE_MOUSE_CONNS.lock().unwrap().contains(&conn)
}

/// Clears the relative mouse mode state for a connection.
///
/// This must be called when an authenticated connection is dropped (during connection teardown)
/// to avoid leaking the connection id in `RELATIVE_MOUSE_CONNS` (a `Mutex<HashSet<i32>>`).
/// Callers are responsible for invoking this on disconnect.
#[inline]
pub(crate) fn clear_relative_mouse_active(conn: i32) {
    set_relative_mouse_active(conn, false);
}

pub(super) static EXITING: AtomicBool = AtomicBool::new(false);

pub(super) static RECORD_CURSOR_POS_RUNNING: AtomicBool = AtomicBool::new(false);

// https://github.com/rustdesk/rustdesk/issues/9729
// We need to do some special handling for macOS when using the legacy mode.
#[cfg(target_os = "macos")]
pub(super) static LAST_KEY_LEGACY_MODE: AtomicBool = AtomicBool::new(true);
// We use enigo to
// 1. Simulate mouse events
// 2. Simulate the legacy mode key events
// 3. Simulate the functioin key events, like LockScreen
#[inline]
#[cfg(target_os = "macos")]
pub(super) fn enigo_ignore_flags() -> bool {
    !LAST_KEY_LEGACY_MODE.load(Ordering::SeqCst)
}
#[inline]
#[cfg(target_os = "macos")]
pub(super) fn set_last_legacy_mode(v: bool) {
    LAST_KEY_LEGACY_MODE.store(v, Ordering::SeqCst);
    ENIGO.lock().unwrap().set_ignore_flags(!v);
}

pub fn try_start_record_cursor_pos() -> Option<thread::JoinHandle<()>> {
    if RECORD_CURSOR_POS_RUNNING.load(Ordering::SeqCst) {
        return None;
    }

    RECORD_CURSOR_POS_RUNNING.store(true, Ordering::SeqCst);
    let handle = thread::spawn(|| {
        let interval = time::Duration::from_millis(33);
        loop {
            if !RECORD_CURSOR_POS_RUNNING.load(Ordering::SeqCst) {
                break;
            }

            let now = time::Instant::now();
            if let Some((x, y)) = crate::get_cursor_pos() {
                update_last_cursor_pos(x, y);
            }
            let elapsed = now.elapsed();
            if elapsed < interval {
                thread::sleep(interval - elapsed);
            }
        }
        update_last_cursor_pos(INVALID_CURSOR_POS, INVALID_CURSOR_POS);
    });
    Some(handle)
}

pub fn try_stop_record_cursor_pos() {
    let remote_count = AUTHED_CONNS
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.conn_type == AuthConnType::Remote)
        .count();
    if remote_count > 0 {
        return;
    }
    RECORD_CURSOR_POS_RUNNING.store(false, Ordering::SeqCst);
}
