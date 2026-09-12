use super::sanitize_filename_component;

#[test]
fn sanitize_recording_filename_component() {
    assert_eq!(
        sanitize_filename_component("192.168.1.2:21118"),
        "192.168.1.2_21118"
    );
    assert_eq!(
        sanitize_filename_component("[2001:db8::1]:21118"),
        "[2001_db8__1]_21118"
    );
    assert_eq!(
        sanitize_filename_component("peer/name\\with?bad\nchars"),
        "peer_name_with_bad_chars"
    );
}
