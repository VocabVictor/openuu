#[test]
fn test_extract_placeholders() {
    use super::extract_placeholder as f;

    assert_eq!(f(""), ("".to_string(), None));
    assert_eq!(
        f("{3} sessions"),
        ("{} sessions".to_string(), Some("3".to_string()))
    );
    assert_eq!(f(" } { "), (" } { ".to_string(), None));
    // Allow empty value
    assert_eq!(
        f("{} sessions"),
        ("{} sessions".to_string(), Some("".to_string()))
    );
    // Match only the first one
    assert_eq!(
        f("{2} times {4} makes {8}"),
        ("{} times {4} makes {8}".to_string(), Some("2".to_string()))
    );
}

#[test]
fn test_resolve_lang_forces_english_for_saved_cjk_when_target_disables_cjk() {
    use super::resolve_lang as f;

    assert_eq!(f("zh-cn", "en-US", true), "en");
    assert_eq!(f("zh-tw", "en-US", true), "en");
    assert_eq!(f("ja", "en-US", true), "en");
    assert_eq!(f("ko", "en-US", true), "en");
}

#[test]
fn test_resolve_lang_forces_english_for_cjk_locale_when_target_disables_cjk() {
    use super::resolve_lang as f;

    assert_eq!(f("", "zh_CN", true), "en");
    assert_eq!(f("", "ja-JP", true), "en");
    assert_eq!(f("", "ko_KR", true), "en");
}

#[test]
fn test_resolve_lang_preserves_cjk_when_target_allows_cjk() {
    use super::resolve_lang as f;

    assert_eq!(f("zh-cn", "en-US", false), "zh-cn");
    assert_eq!(f("", "zh_TW", false), "zh-tw");
    assert_eq!(f("", "ja-JP", false), "ja");
}
