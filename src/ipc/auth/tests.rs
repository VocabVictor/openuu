#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn test_service_peer_uid_policy() {
    assert!(super::is_allowed_service_peer_uid(0, None));
    assert!(super::is_allowed_service_peer_uid(501, Some(501)));
    assert!(!super::is_allowed_service_peer_uid(502, Some(501)));
    assert!(!super::is_allowed_service_peer_uid(501, None));
}

#[test]
#[cfg(windows)]
fn test_windows_server_peer_policy() {
    assert!(super::is_allowed_windows_session_scoped_peer(
        true, None, None
    ));
    assert!(super::is_allowed_windows_session_scoped_peer(
        false,
        Some(1),
        Some(1)
    ));
    assert!(!super::is_allowed_windows_session_scoped_peer(
        false,
        Some(1),
        Some(2)
    ));
    assert!(!super::is_allowed_windows_session_scoped_peer(
        false,
        None,
        Some(1)
    ));
}

#[test]
#[cfg(windows)]
fn test_windows_portable_service_peer_policy() {
    assert!(super::is_allowed_windows_portable_service_peer(
        Some(true),
        None,
        None
    ));
    assert!(!super::is_allowed_windows_portable_service_peer(
        Some(false),
        Some(1),
        Some(1)
    ));
    assert!(!super::is_allowed_windows_portable_service_peer(
        Some(false),
        Some(1),
        Some(2)
    ));
    assert!(!super::is_allowed_windows_portable_service_peer(
        None,
        Some(1),
        Some(1)
    ));
}

#[test]
#[cfg(windows)]
fn test_should_allow_everyone_create_on_windows_policy() {
    assert!(super::should_allow_everyone_create_on_windows(""));
    assert!(super::should_allow_everyone_create_on_windows("_service"));
    assert!(!super::should_allow_everyone_create_on_windows(
        "_portable_service"
    ));
}

#[test]
#[cfg(windows)]
fn test_executable_paths_match_windows_normalization() {
    let left = std::path::PathBuf::from(r"\\?\C:\Program Files\RustDesk\RustDesk.exe");
    let right = std::path::PathBuf::from(r"c:\program files\rustdesk\rustdesk.exe");
    assert!(super::executable_paths_match(&left, &right));
}

#[test]
#[cfg(target_os = "macos")]
fn test_os_str_eq_ignore_ascii_case_for_process_names() {
    assert!(super::os_str_eq_ignore_ascii_case(
        Some(std::ffi::OsStr::new("RustDesk")),
        Some(std::ffi::OsStr::new("rustdesk"))
    ));
    assert!(!super::os_str_eq_ignore_ascii_case(
        Some(std::ffi::OsStr::new("RustDesk")),
        Some(std::ffi::OsStr::new("service"))
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn test_console_owner_uid_matches_get_active_userid() {
    let console_uid =
        super::console_owner_uid().expect("/dev/console must have a resolvable uid");
    let raw_uid = crate::platform::macos::get_active_userid();
    let parsed_uid: u32 = raw_uid
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("failed to parse get_active_userid() output: '{raw_uid}'"));
    assert_eq!(parsed_uid, console_uid);
}
