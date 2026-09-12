use super::*;

    #[test]
    fn test_is_public() {
        // Test URLs containing "rustdesk.com/"
        assert!(is_public("https://rustdesk.com/"));
        assert!(is_public("https://www.rustdesk.com/"));
        assert!(is_public("https://api.rustdesk.com/v1"));
        assert!(is_public("https://API.RUSTDESK.COM/v1"));
        assert!(is_public("https://rustdesk.com/path"));

        // Test URLs ending with "rustdesk.com"
        assert!(is_public("rustdesk.com"));
        assert!(is_public("https://rustdesk.com"));
        assert!(is_public("https://RustDesk.com"));
        assert!(is_public("http://www.rustdesk.com"));
        assert!(is_public("https://api.rustdesk.com"));

        // Test non-public URLs
        assert!(!is_public("https://example.com"));
        assert!(!is_public("https://custom-server.com"));
        assert!(!is_public("http://192.168.1.1"));
        assert!(!is_public("localhost"));
        assert!(!is_public("https://rustdesk.computer.com"));
        assert!(!is_public("rustdesk.comhello.com"));
    }

    #[test]
    fn test_is_public_matches_rustdesk_root_domain() {
        assert!(is_public("rustdesk.com/"));
        assert!(is_public("rustdesk.com:21117"));
        assert!(is_public("api.rustdesk.com:21117"));
        assert!(!is_public("hello-rustdesk.com"));
        assert!(!is_public("api.rustdesk.com.evil.test"));
        assert!(!is_public("https://rustdesk.com@evil.test"));
    }
