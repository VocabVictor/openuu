use super::{is_valid_capture_frame_length, ADDR_CAPTURE_FRAME};

#[test]
fn test_is_valid_capture_frame_length_rejects_zero_length() {
    assert!(!is_valid_capture_frame_length(ADDR_CAPTURE_FRAME + 1024, 0));
}

#[test]
fn test_is_valid_capture_frame_length_rejects_out_of_bounds_length() {
    assert!(!is_valid_capture_frame_length(ADDR_CAPTURE_FRAME + 16, 17));
}

#[test]
fn test_is_valid_capture_frame_length_accepts_in_bounds_length() {
    assert!(is_valid_capture_frame_length(ADDR_CAPTURE_FRAME + 16, 16));
}
