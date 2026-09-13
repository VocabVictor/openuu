use super::*;

/// Input text via clipboard + Shift+Insert.
/// Shift+Insert is more universal than Ctrl+V, works in both GUI apps and terminals.
///
/// Note: Clipboard content is NOT restored after paste - see `set_clipboard_for_paste_sync` for rationale.
pub(super) fn input_text_via_clipboard(text: &str, conn: Arc<SyncConnection>, session: &Path<'static>) {
    if text.is_empty() {
        return;
    }
    if !set_clipboard_for_paste_sync(text) {
        return;
    }

    let portal = get_portal(&conn);
    let shift_keycode = evdev::Key::KEY_LEFTSHIFT.code() as i32;
    let insert_keycode = evdev::Key::KEY_INSERT.code() as i32;

    // Send Shift+Insert (universal paste shortcut)
    if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
        &portal,
        session,
        HashMap::new(),
        shift_keycode,
        PRESSED_DOWN_STATE,
    ) {
        log::error!("input_text_via_clipboard: failed to press Shift: {:?}", e);
        return;
    }

    // Press Insert
    if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
        &portal,
        session,
        HashMap::new(),
        insert_keycode,
        PRESSED_DOWN_STATE,
    ) {
        log::error!("input_text_via_clipboard: failed to press Insert: {:?}", e);
        // Still try to release Shift.
        // Note: clipboard has already been set by set_clipboard_for_paste_sync but paste
        // never happened. We don't attempt to restore the previous clipboard contents
        // because reading the clipboard on Wayland requires focus/permission.
        let _ = remote_desktop_portal::notify_keyboard_keycode(
            &portal,
            session,
            HashMap::new(),
            shift_keycode,
            PRESSED_UP_STATE,
        );
        return;
    }

    // Release Insert
    if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
        &portal,
        session,
        HashMap::new(),
        insert_keycode,
        PRESSED_UP_STATE,
    ) {
        log::error!(
            "input_text_via_clipboard: failed to release Insert: {:?}",
            e
        );
    }

    // Release Shift
    if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
        &portal,
        session,
        HashMap::new(),
        shift_keycode,
        PRESSED_UP_STATE,
    ) {
        log::error!("input_text_via_clipboard: failed to release Shift: {:?}", e);
    }
}

pub(super) fn desktop_is_niri(desktop: &str) -> bool {
    desktop
        .split(':')
        .any(|name| name.eq_ignore_ascii_case("niri"))
}
