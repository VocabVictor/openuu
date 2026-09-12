use super::*;

/// Encode a message for the helper protocol.
/// Format: [type: u8][length: u32 LE][payload: bytes]
pub fn encode_helper_message(msg_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(MSG_HEADER_SIZE + payload.len());
    msg.push(msg_type);
    msg.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    msg.extend_from_slice(payload);
    msg
}

/// Encode a resize message for the helper protocol.
/// Payload: rows (u16 LE) + cols (u16 LE)
pub fn encode_resize_message(rows: u16, cols: u16) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4);
    payload.extend_from_slice(&rows.to_le_bytes());
    payload.extend_from_slice(&cols.to_le_bytes());
    encode_helper_message(MSG_TYPE_RESIZE, &payload)
}
