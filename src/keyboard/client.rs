use super::*;

mod grab;
pub use grab::*;

/// Tracks grab ownership and serializes transitions across threads.
///
/// Multiple Flutter isolates (one per session window) call
/// `change_grab_status(Run/Wait)` concurrently. Without serialization a
/// stale `Wait` from session A can clobber session B's freshly acquired
/// grab on any desktop OS.
///
/// Windows and macOS are less susceptible in practice because the Flutter
/// side triggers `enterView` only after a mouse click inside the window,
/// but we cannot rely on that. On Linux/X11, `XGrabKeyboard` can also
/// cause a focus-change feedback loop (~10 Hz), so `last_grab` debounces
/// spurious `Wait` events that arrive shortly after a `Run`.
#[derive(Default)]
struct GrabOwnerState {
    owner: Option<u128>,
    last_grab: Option<std::time::Instant>,
    /// True while a deferred-release thread is in flight. Prevents
    /// spawning redundant threads during the X11 feedback loop.
    deferred_pending: bool,
}

/// How long after a grab acquisition we suppress Wait from the same session.
/// Must exceed one full X11 feedback cycle (~100 ms: 50 ms enable + 50 ms disable).
#[cfg(target_os = "linux")]
const GRAB_DEBOUNCE_MS: u128 = 300;

lazy_static::lazy_static! {
    static ref IS_GRAB_STARTED: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref GRAB_STATE: Arc<Mutex<GrabOwnerState>> = Arc::new(Mutex::new(GrabOwnerState::default()));
}

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    static ref GRAB_OP_LOCK: Mutex<()> = Mutex::new(());
}

#[cfg(target_os = "linux")]
fn apply_run_grab_if_owner(session_id: u128, disable_first: bool) {
    let _lock = GRAB_OP_LOCK.lock().unwrap();
    let gs = GRAB_STATE.lock().unwrap();
    if gs.owner != Some(session_id) {
        return;
    }
    drop(gs);
    if disable_first {
        log::debug!("[grab] handoff: disable_grab before re-grab");
        rdev::disable_grab();
    }
    rdev::enable_grab();
}

#[cfg(target_os = "linux")]
fn disable_grab_if_released() {
    let _lock = GRAB_OP_LOCK.lock().unwrap();
    let should_disable = {
        let gs = GRAB_STATE.lock().unwrap();
        gs.owner.is_none() && gs.last_grab.is_none()
    };
    if should_disable {
        rdev::disable_grab();
    }
}

pub fn start_grab_loop() {
    let mut lock = IS_GRAB_STARTED.lock().unwrap();
    if *lock {
        return;
    }
    super::start_grab_loop();
    *lock = true;
}

pub fn process_event(keyboard_mode: &str, event: &Event, lock_modes: Option<i32>) {
    let keyboard_mode = get_keyboard_mode_enum(keyboard_mode);
    if is_long_press(&event) {
        return;
    }
    let peer = get_peer_platform().to_lowercase();
    for key_event in event_to_key_events(peer, &event, keyboard_mode, lock_modes) {
        send_key_event(&key_event);
    }
}

pub fn process_event_with_session<T: InvokeUiSession>(
    keyboard_mode: &str,
    event: &Event,
    lock_modes: Option<i32>,
    session: &Session<T>,
) {
    let keyboard_mode = get_keyboard_mode_enum(keyboard_mode);
    if is_long_press(&event) {
        return;
    }
    let peer = session.peer_platform().to_lowercase();
    for key_event in event_to_key_events(peer, &event, keyboard_mode, lock_modes) {
        session.send_key_event(&key_event);
    }
}

pub fn get_modifiers_state(
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) -> (bool, bool, bool, bool) {
    let modifiers_lock = MODIFIERS_STATE.lock().unwrap();
    let ctrl = *modifiers_lock.get(&Key::ControlLeft).unwrap()
        || *modifiers_lock.get(&Key::ControlRight).unwrap()
        || ctrl;
    let shift = *modifiers_lock.get(&Key::ShiftLeft).unwrap()
        || *modifiers_lock.get(&Key::ShiftRight).unwrap()
        || shift;
    let command = *modifiers_lock.get(&Key::MetaLeft).unwrap()
        || *modifiers_lock.get(&Key::MetaRight).unwrap()
        || command;
    let alt = *modifiers_lock.get(&Key::Alt).unwrap()
        || *modifiers_lock.get(&Key::AltGr).unwrap()
        || alt;

    (alt, ctrl, shift, command)
}

pub fn legacy_modifiers(
    key_event: &mut KeyEvent,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    if alt
        && !crate::is_control_key(&key_event, &ControlKey::Alt)
        && !crate::is_control_key(&key_event, &ControlKey::RAlt)
    {
        key_event.modifiers.push(ControlKey::Alt.into());
    }
    if shift
        && !crate::is_control_key(&key_event, &ControlKey::Shift)
        && !crate::is_control_key(&key_event, &ControlKey::RShift)
    {
        key_event.modifiers.push(ControlKey::Shift.into());
    }
    if ctrl
        && !crate::is_control_key(&key_event, &ControlKey::Control)
        && !crate::is_control_key(&key_event, &ControlKey::RControl)
    {
        key_event.modifiers.push(ControlKey::Control.into());
    }
    if command
        && !crate::is_control_key(&key_event, &ControlKey::Meta)
        && !crate::is_control_key(&key_event, &ControlKey::RWin)
    {
        key_event.modifiers.push(ControlKey::Meta.into());
    }
}

#[cfg(target_os = "android")]
pub fn map_key_to_control_key(key: &rdev::Key) -> Option<ControlKey> {
    match key {
        Key::Alt => Some(ControlKey::Alt),
        Key::ShiftLeft => Some(ControlKey::Shift),
        Key::ControlLeft => Some(ControlKey::Control),
        Key::MetaLeft => Some(ControlKey::Meta),
        Key::AltGr => Some(ControlKey::RAlt),
        Key::ShiftRight => Some(ControlKey::RShift),
        Key::ControlRight => Some(ControlKey::RControl),
        Key::MetaRight => Some(ControlKey::RWin),
        _ => None,
    }
}

pub fn event_lock_screen() -> KeyEvent {
    let mut key_event = KeyEvent::new();
    key_event.set_control_key(ControlKey::LockScreen);
    key_event.down = true;
    key_event.mode = KeyboardMode::Legacy.into();
    key_event
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn lock_screen() {
    send_key_event(&event_lock_screen());
}

pub fn event_ctrl_alt_del() -> KeyEvent {
    let mut key_event = KeyEvent::new();
    if get_peer_platform() == "Windows" {
        key_event.set_control_key(ControlKey::CtrlAltDel);
        key_event.down = true;
    } else {
        key_event.set_control_key(ControlKey::Delete);
        legacy_modifiers(&mut key_event, true, true, false, false);
        key_event.press = true;
    }
    key_event.mode = KeyboardMode::Legacy.into();
    key_event
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn ctrl_alt_del() {
    send_key_event(&event_ctrl_alt_del());
}
