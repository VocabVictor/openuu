use super::*;
use super::super::http_request::http_request_via_tcp_proxy;

    #[test]
    fn test_should_use_tcp_proxy_for_api_url() {
        assert!(should_use_tcp_proxy_for_api_url(
            "https://admin.example.com/api/login",
            "https://admin.example.com"
        ));
        assert!(should_use_tcp_proxy_for_api_url(
            "https://admin.example.com:21114/api/login",
            "https://admin.example.com"
        ));
        assert!(!should_use_tcp_proxy_for_api_url(
            "https://api.telegram.org/bot123/sendMessage",
            "https://admin.example.com"
        ));
        assert!(!should_use_tcp_proxy_for_api_url(
            "https://admin.rustdesk.com/api/login",
            "https://admin.rustdesk.com"
        ));
        assert!(!should_use_tcp_proxy_for_api_url(
            "https://admin.example.com/api/login",
            "not a url"
        ));
        assert!(!should_use_tcp_proxy_for_api_url(
            "not a url",
            "https://admin.example.com"
        ));
    }

    #[test]
    fn test_get_tcp_proxy_addr_normalizes_bare_ipv6_host() {
        struct RestoreCustomRendezvousServer(String);

        impl Drop for RestoreCustomRendezvousServer {
            fn drop(&mut self) {
                Config::set_option(
                    keys::OPTION_CUSTOM_RENDEZVOUS_SERVER.to_string(),
                    self.0.clone(),
                );
            }
        }

        let _restore = RestoreCustomRendezvousServer(Config::get_option(
            keys::OPTION_CUSTOM_RENDEZVOUS_SERVER,
        ));
        Config::set_option(
            keys::OPTION_CUSTOM_RENDEZVOUS_SERVER.to_string(),
            "1:2".to_string(),
        );

        assert_eq!(get_tcp_proxy_addr(), format!("[1:2]:{RENDEZVOUS_PORT}"));
    }

    #[tokio::test]
    async fn test_http_request_via_tcp_proxy_rejects_invalid_header_json() {
        let result = http_request_via_tcp_proxy("not a url", "get", None, "{").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_http_request_via_tcp_proxy_rejects_non_object_header_json() {
        let err = http_request_via_tcp_proxy("not a url", "get", None, "[]")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("HTTP header information parsing failed!"));
    }

    #[test]
    fn test_parse_json_header_entries_preserves_single_content_type() {
        let headers = parse_json_header_entries(
            r#"{"Content-Type":"text/plain","Authorization":"Bearer token"}"#,
        )
        .unwrap();

        assert_eq!(
            headers
                .iter()
                .filter(|entry| entry.name.eq_ignore_ascii_case("Content-Type"))
                .count(),
            1
        );
        assert_eq!(
            headers
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case("Content-Type"))
                .map(|entry| entry.value.as_str()),
            Some("text/plain")
        );
    }

    #[test]
    fn test_parse_json_header_entries_does_not_add_default_content_type() {
        let headers = parse_json_header_entries(r#"{"Authorization":"Bearer token"}"#).unwrap();

        assert!(!headers
            .iter()
            .any(|entry| entry.name.eq_ignore_ascii_case("Content-Type")));
    }

    #[test]
    fn test_parse_simple_header_respects_custom_content_type() {
        let headers = parse_simple_header("Content-Type: text/plain");

        assert_eq!(
            headers
                .iter()
                .filter(|entry| entry.name.eq_ignore_ascii_case("Content-Type"))
                .count(),
            1
        );
        assert_eq!(
            headers
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case("Content-Type"))
                .map(|entry| entry.value.as_str()),
            Some("text/plain")
        );
    }

    #[test]
    fn test_parse_simple_header_preserves_non_content_type_header() {
        let headers = parse_simple_header("Authorization: Bearer token");

        assert!(headers.iter().any(|entry| {
            entry.name.eq_ignore_ascii_case("Authorization")
                && entry.value.as_str() == "Bearer token"
        }));
        assert_eq!(
            headers
                .iter()
                .filter(|entry| entry.name.eq_ignore_ascii_case("Content-Type"))
                .count(),
            1
        );
        assert_eq!(
            headers
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case("Content-Type"))
                .map(|entry| entry.value.as_str()),
            Some("application/json")
        );
    }

    #[test]
    fn test_tcp_proxy_log_target_redacts_query_only() {
        assert_eq!(
            tcp_proxy_log_target("https://example.com/api/heartbeat?token=secret"),
            "https://example.com/api/heartbeat"
        );
    }

    #[test]
    fn test_tcp_proxy_log_target_brackets_ipv6_host_with_port() {
        assert_eq!(
            tcp_proxy_log_target("https://[2001:db8::1]:21114/api/heartbeat?token=secret"),
            "https://[2001:db8::1]:21114/api/heartbeat"
        );
    }

    #[test]
    fn test_http_proxy_response_to_json() {
        let mut resp = HttpProxyResponse {
            status: 200,
            body: br#"{"ok":true}"#.to_vec().into(),
            ..Default::default()
        };
        resp.headers.push(HeaderEntry {
            name: "Content-Type".into(),
            value: "application/json".into(),
            ..Default::default()
        });

        let json = http_proxy_response_to_json(resp).unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["status_code"], 200);
        assert_eq!(value["headers"]["content-type"], "application/json");
        assert_eq!(value["body"], r#"{"ok":true}"#);

        let err = http_proxy_response_to_json(HttpProxyResponse {
            error: "dial failed".into(),
            ..Default::default()
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("TCP proxy error: dial failed"));
    }
