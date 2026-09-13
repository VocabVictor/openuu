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

    /// Hole punching used to be switched off for every self-hosted deployment,
    /// which read a deployment's shape as a statement about its capability. Only
    /// WebRTC still defaults off, and only while the deployment has no ICE
    /// servers of its own, because it would otherwise fall back to the built-in
    /// public STUN list.
    #[test]
    fn capabilities_that_need_stun_wait_for_ice_servers() {
        // IPv6 punching reaches the public STUN list too, so it is governed
        // alongside WebRTC and not with plain UDP punching
        assert!(needs_own_ice_servers(keys::OPTION_ENABLE_WEBRTC));
        assert!(needs_own_ice_servers(keys::OPTION_ENABLE_IPV6_PUNCH));
        assert!(!needs_own_ice_servers(keys::OPTION_ENABLE_UDP_PUNCH));

        // unset + self-hosted + no ICE servers of its own: the one case that stays off
        assert!(off_by_default("", false, false));

        // any one of those three conditions lifting is enough to honour the default
        assert!(!off_by_default("", true, false), "a public server has ICE behind it");
        assert!(!off_by_default("", false, true), "the deployment configured its own");
        assert!(!off_by_default("Y", false, false), "an explicit choice is not a default");
        assert!(!off_by_default("N", false, false), "an explicit no is already no");
    }
