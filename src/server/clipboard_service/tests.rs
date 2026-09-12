use super::{decode_utf8_prefix, WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES};

#[test]
fn decode_utf8_prefix_returns_text_for_valid_utf8() {
    let text = "hello-مرحبا";
    assert_eq!(decode_utf8_prefix(text.as_bytes()), Some(text.to_owned()));
}

#[test]
fn decode_utf8_prefix_returns_none_for_invalid_utf8_sequence() {
    let bytes = b"ab\xffcd";
    assert_eq!(decode_utf8_prefix(bytes), None);
}

#[test]
fn decode_utf8_prefix_trims_incomplete_utf8_suffix() {
    let bytes = vec![b'a', 0xE4, 0xB8];
    assert_eq!(decode_utf8_prefix(&bytes), Some("a".to_owned()));
}

#[test]
fn decode_utf8_prefix_applies_max_bytes_limit() {
    let bytes = vec![b'a'; WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES + 8];
    let result = decode_utf8_prefix(&bytes).expect("expected decoded prefix");
    assert_eq!(result.len(), WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES);
}

#[test]
fn decode_utf8_prefix_keeps_utf8_boundary_when_limited() {
    let mut bytes = vec![b'a'; WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES - 1];
    bytes.extend_from_slice("ا".as_bytes());
    let result = decode_utf8_prefix(&bytes).expect("expected decoded prefix");
    assert_eq!(
        result.len(),
        WAYLAND_CLIPBOARD_SKIP_CHECK_MAX_UTF8_BYTES - 1
    );
    assert!(result.chars().all(|c| c == 'a'));
}
