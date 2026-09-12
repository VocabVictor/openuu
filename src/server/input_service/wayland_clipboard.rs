use super::*;

/// Delay in milliseconds to wait for clipboard to sync on Wayland.
/// This is an empirical value — Wayland provides no callback or event to confirm
/// clipboard content has been received by the compositor. Under heavy system load,
/// this delay may be insufficient, but there is no reliable alternative mechanism.
#[cfg(target_os = "linux")]
pub(super) const CLIPBOARD_SYNC_DELAY_MS: u64 = 50;
#[cfg(target_os = "linux")]
pub(super) const WAYLAND_CLIPBOARD_INPUT_FILTER_WINDOW: Duration = Duration::from_secs(1);
#[cfg(target_os = "linux")]
pub(super) const WAYLAND_CLIPBOARD_INPUT_MAX_RECORDS: usize = 256;
#[cfg(target_os = "linux")]
pub(super) const WAYLAND_CLIPBOARD_INPUT_MAX_TEXT_CHARS: usize = 1024;

#[cfg(target_os = "linux")]
pub(super) fn cleanup_wayland_clipboard_input_records(records: &mut Vec<(Instant, String)>, now: Instant) {
    records.retain(|(created_at, _)| {
        now.saturating_duration_since(*created_at) <= WAYLAND_CLIPBOARD_INPUT_FILTER_WINDOW
    });
    let len = records.len();
    if len > WAYLAND_CLIPBOARD_INPUT_MAX_RECORDS {
        records.drain(0..(len - WAYLAND_CLIPBOARD_INPUT_MAX_RECORDS));
    }
}

#[cfg(target_os = "linux")]
#[inline]
pub(super) fn normalize_wayland_clipboard_input_text(text: &str) -> String {
    text.chars()
        .take(WAYLAND_CLIPBOARD_INPUT_MAX_TEXT_CHARS)
        .collect()
}

#[cfg(target_os = "linux")]
#[inline]
pub(super) fn get_wayland_clipboard_input_normalized_text(text: &str) -> Option<String> {
    let normalized = normalize_wayland_clipboard_input_text(text);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

#[cfg(target_os = "linux")]
#[inline]
pub(super) fn record_wayland_clipboard_input_for_sync_filter(text: &str) -> Option<(Instant, String)> {
    if text.is_empty() || crate::platform::linux::is_x11() {
        return None;
    }
    let normalized = get_wayland_clipboard_input_normalized_text(text)?;
    let now = Instant::now();
    let mut records = WAYLAND_CLIPBOARD_INPUT_RECORDS.lock().unwrap();
    cleanup_wayland_clipboard_input_records(&mut records, now);
    records.push((now, normalized.clone()));
    Some((now, normalized))
}

#[cfg(target_os = "linux")]
#[inline]
pub(super) fn rollback_wayland_clipboard_input_record(record: (Instant, String)) {
    let (created_at, normalized) = record;
    let now = Instant::now();
    let mut records = WAYLAND_CLIPBOARD_INPUT_RECORDS.lock().unwrap();
    cleanup_wayland_clipboard_input_records(&mut records, now);
    if let Some(pos) = records
        .iter()
        .rposition(|(record_created_at, record_normalized)| {
            *record_created_at == created_at && *record_normalized == normalized
        })
    {
        records.remove(pos);
    }
}

#[cfg(target_os = "linux")]
pub(super) fn is_recent_wayland_clipboard_input(text: &str) -> bool {
    if text.is_empty() || crate::platform::linux::is_x11() {
        return false;
    }
    let Some(normalized) = get_wayland_clipboard_input_normalized_text(text) else {
        return false;
    };
    let now = Instant::now();
    let mut records = WAYLAND_CLIPBOARD_INPUT_RECORDS.lock().unwrap();
    cleanup_wayland_clipboard_input_records(&mut records, now);
    records
        .iter()
        .any(|(_, record_normalized)| record_normalized == &normalized)
}

/// Internal: Set clipboard content without delay.
/// Returns true if clipboard was set successfully.
#[cfg(target_os = "linux")]
pub(super) fn set_clipboard_content(text: &str) -> bool {
    if let Err(e) = crate::clipboard::set_text_clipboard_with_owner_sync(
        text,
        crate::clipboard::ClipboardSide::Host,
    ) {
        log::error!(
            "set_clipboard_content: failed to set clipboard with owner marker: {:?}",
            e
        );
        return false;
    }
    true
}

/// Set clipboard content for paste operation (sync version for use in blocking contexts).
///
/// Note: The original clipboard content is intentionally NOT restored after paste.
/// Restoring clipboard could cause race conditions where subsequent keystrokes
/// might accidentally paste the old clipboard content instead of the intended input.
/// This trade-off prioritizes input reliability over preserving clipboard state.
#[cfg(target_os = "linux")]
#[inline]
pub(super) fn set_clipboard_for_paste_sync(text: &str) -> bool {
    let record = record_wayland_clipboard_input_for_sync_filter(text);
    if !set_clipboard_content(text) {
        if let Some(record) = record {
            rollback_wayland_clipboard_input_record(record);
        }
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(CLIPBOARD_SYNC_DELAY_MS));
    true
}

/// Check if a character is ASCII printable (0x20-0x7E).
#[cfg(target_os = "linux")]
#[inline]
pub(super) fn is_ascii_printable(c: char) -> bool {
    c as u32 >= 0x20 && c as u32 <= 0x7E
}

/// Input a single character via clipboard + Shift+Insert in server process.
#[cfg(target_os = "linux")]
#[inline]
pub(super) fn input_char_via_clipboard_server(en: &mut Enigo, chr: char) {
    input_text_via_clipboard_server(en, &chr.to_string());
}

/// Input text via clipboard + Shift+Insert in server process.
/// Shift+Insert is more universal than Ctrl+V, works in both GUI apps and terminals.
///
/// Note: Clipboard content is NOT restored after paste - see `set_clipboard_for_paste_sync` for rationale.
#[cfg(target_os = "linux")]
pub(super) fn input_text_via_clipboard_server(en: &mut Enigo, text: &str) {
    if text.is_empty() {
        return;
    }
    if !set_clipboard_for_paste_sync(text) {
        return;
    }

    // Use ENIGO's custom_keyboard directly to avoid creating new IPC connections
    // which would cause excessive logging and keyboard device creation/destruction
    if en.key_down(Key::Shift).is_err() {
        log::error!("input_text_via_clipboard_server: failed to press Shift, skipping paste");
        return;
    }
    if en.key_down(Key::Raw(XKB_KEY_INSERT)).is_err() {
        log::error!("input_text_via_clipboard_server: failed to press Insert, releasing Shift");
        en.key_up(Key::Shift);
        return;
    }
    en.key_up(Key::Raw(XKB_KEY_INSERT));
    en.key_up(Key::Shift);

    // Brief delay to allow the target application to process the paste event.
    // Empirical value — no reliable synchronization mechanism exists on Wayland.
    std::thread::sleep(std::time::Duration::from_millis(20));
}

/// Check if any hotkey modifier (Ctrl/Alt/Meta) is currently pressed.
/// Used to detect hotkey combinations like Ctrl+C, Alt+Tab, etc.
///
/// Note: Shift is intentionally NOT checked here. Shift+character produces a different
/// character (e.g., Shift+a → 'A'), which is normal text input, not a hotkey.
/// Shift is only relevant as a hotkey modifier when combined with Ctrl/Alt/Meta
/// (e.g., Ctrl+Shift+Z), in which case this function already returns true via Ctrl.
#[cfg(target_os = "linux")]
#[inline]
pub(super) fn is_hotkey_modifier_pressed(en: &mut Enigo) -> bool {
    get_modifier_state(Key::Control, en)
        || get_modifier_state(Key::RightControl, en)
        || get_modifier_state(Key::Alt, en)
        || get_modifier_state(Key::RightAlt, en)
        || get_modifier_state(Key::Meta, en)
        || get_modifier_state(Key::RWin, en)
}

/// Release Shift keys before character input in Legacy/Translate mode.
/// In these modes, the character has already been converted by the client,
/// so we should input it directly without Shift modifier affecting the result.
///
/// Note: Does NOT release Shift if hotkey modifiers (Ctrl/Alt/Meta) are pressed,
/// to preserve combinations like Ctrl+Shift+Z.
#[cfg(target_os = "linux")]
pub(super) fn release_shift_for_char_input(en: &mut Enigo) {
    // Don't release Shift if hotkey modifiers (Ctrl/Alt/Meta) are pressed.
    // This preserves combinations like Ctrl+Shift+Z.
    if is_hotkey_modifier_pressed(en) {
        return;
    }

    // In translate mode, the client has already converted the keystroke to a character
    // (e.g., Shift+a → 'A'). We release Shift here so the server inputs the character
    // directly without Shift affecting the result.
    //
    // Shift is intentionally NOT restored after input — the client will send an explicit
    // Shift key_up event when the user physically releases Shift. Restoring it here would
    // cause a brief Shift re-press that could interfere with the next input event.

    let is_x11 = crate::platform::linux::is_x11();

    if get_modifier_state(Key::Shift, en) {
        if !is_x11 {
            en.key_up(Key::Shift);
        } else {
            simulate_(&EventType::KeyRelease(RdevKey::ShiftLeft));
        }
    }
    if get_modifier_state(Key::RightShift, en) {
        if !is_x11 {
            en.key_up(Key::RightShift);
        } else {
            simulate_(&EventType::KeyRelease(RdevKey::ShiftRight));
        }
    }
}
