use super::*;

#[test]
fn resolve_within_rejects_paths_that_escape() {
    let base = Path::new("/base");
    assert_eq!(
        resolve_within(base, "./data/logo.png"),
        Some(base.join("data").join("logo.png"))
    );
    assert_eq!(
        resolve_within(base, ".\\data\\logo.png"),
        Some(base.join("data").join("logo.png"))
    );
    // meta.toml is user-writable, so these must not reach remove_file.
    assert_eq!(resolve_within(base, "../../etc/passwd"), None);
    assert_eq!(resolve_within(base, "/etc/passwd"), None);
    assert_eq!(resolve_within(base, "C:\\Windows\\System32\\x.dll"), None);
    assert_eq!(resolve_within(base, "."), None);
    assert_eq!(resolve_within(base, ""), None);
}
