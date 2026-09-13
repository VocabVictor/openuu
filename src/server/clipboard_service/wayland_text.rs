use super::*;

#[cfg(target_os = "linux")]
pub(super) const WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES: usize =
    super::input_service::WAYLAND_CLIPBOARD_INPUT_MAX_TEXT_CHARS * 4;

#[cfg(target_os = "linux")]
pub(super) fn decode_utf8_prefix(bytes: &[u8]) -> Option<String> {
    let end = bytes.len().min(WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES);
    let slice = &bytes[..end];
    match std::str::from_utf8(slice) {
        Ok(text) => Some(text.to_owned()),
        Err(e) => {
            if e.error_len().is_some() {
                return None;
            }
            let valid_up_to = e.valid_up_to();
            std::str::from_utf8(&slice[..valid_up_to])
                .ok()
                .map(ToOwned::to_owned)
        }
    }
}

#[cfg(target_os = "linux")]
pub(super) fn decode_text_clipboard(clipboard: &Clipboard) -> Option<String> {
    if clipboard.format.enum_value() != Ok(ClipboardFormat::Text) {
        return None;
    }
    if clipboard.compress {
        let bytes = hbb_common::compress::decompress(&clipboard.content);
        return decode_utf8_prefix(&bytes);
    }
    decode_utf8_prefix(&clipboard.content)
}

#[cfg(target_os = "linux")]
pub(super) fn should_skip_wayland_clipboard_sync(msg: &Message) -> bool {
    if crate::platform::linux::is_x11() {
        return false;
    }
    let is_recent_wayland_input = |clipboard: &Clipboard| -> bool {
        let Some(text) = decode_text_clipboard(clipboard) else {
            return false;
        };
        super::input_service::is_recent_wayland_clipboard_input(&text)
    };

    match &msg.union {
        Some(message::Union::Clipboard(clipboard)) => is_recent_wayland_input(clipboard),
        Some(message::Union::MultiClipboards(multi_clipboards)) => multi_clipboards
            .clipboards
            .iter()
            .any(is_recent_wayland_input),
        _ => false,
    }
}
