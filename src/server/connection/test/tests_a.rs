use super::*;

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
pub(super) fn test_pending_switch_sides_uuid_is_claimed_once() {
    let id = uuid::Uuid::new_v4().to_string();
    let uuid = uuid::Uuid::new_v4();
    let other_uuid = uuid::Uuid::new_v4();
    assert!(insert_pending_switch_sides_uuid(id.clone(), uuid.clone()));

    assert!(!insert_pending_switch_sides_uuid(id.clone(), uuid.clone()));
    assert!(has_pending_switch_sides_uuid(&id, &uuid));
    assert!(!has_pending_switch_sides_uuid(&id, &other_uuid));
    assert!(!claim_pending_switch_sides_uuid("other-peer", &uuid));
    assert!(!claim_pending_switch_sides_uuid(&id, &other_uuid));
    assert!(claim_pending_switch_sides_uuid(&id, &uuid));
    assert!(!has_pending_switch_sides_uuid(&id, &uuid));
    assert!(!claim_pending_switch_sides_uuid(&id, &uuid));
    assert!(!insert_pending_switch_sides_uuid(id, uuid));
}

#[test]
pub(super) fn login_scope_latches_session_scope_across_login_retries() {
    let port_forward = |host: &str| {
        let mut lr = LoginRequest::new();
        lr.my_id = "peer".to_owned();
        lr.set_port_forward(PortForward {
            host: host.to_owned(),
            port: 3389,
            ..Default::default()
        });
        lr
    };
    let first = port_forward("localhost");
    let scope = |lr: &LoginRequest| Connection::login_scope_digest(lr);

    // A retry may carry new credentials, profile data, options, and unknown fields.
    let mut retry = port_forward("localhost");
    retry.password = "secret".into();
    retry.hwid = "hwid".into();
    retry.os_login = Some(OSLogin {
        username: "admin".to_owned(),
        ..Default::default()
    })
    .into();
    retry.my_name = "New Display Name".to_owned();
    retry.avatar = "data:image/png;base64,AAAA".to_owned();
    retry
        .special_fields
        .mut_unknown_fields()
        .add_varint(9999, 1);
    assert_eq!(scope(&first), scope(&retry));

    // It may not change the controller identity, move the target, or switch type.
    let mut rotated_id = first.clone();
    rotated_id.my_id = "rotated-id".to_owned();
    assert_ne!(scope(&first), scope(&rotated_id));
    assert_ne!(scope(&first), scope(&port_forward("10.0.0.5")));
    let mut moved_port = port_forward("localhost");
    moved_port.mut_port_forward().port = 22;
    assert_ne!(scope(&first), scope(&moved_port));
    let terminal = |service_id: &str| {
        let mut lr = LoginRequest::new();
        lr.my_id = "peer".to_owned();
        lr.set_terminal(Terminal {
            service_id: service_id.to_owned(),
            ..Default::default()
        });
        lr
    };
    assert_ne!(scope(&first), scope(&terminal("")));
    assert_ne!(scope(&terminal("a")), scope(&terminal("b")));
}

#[test]
pub(super) fn test_wildcard_match() {
    // Exact match.
    assert!(wildcard_match("123456789", "123456789"));
    assert!(!wildcard_match("123456789", "123456780"));
    assert!(!wildcard_match("12345678", "123456789"));
    assert!(!wildcard_match("123456789", "12345678"));
    // Case-insensitive.
    assert!(wildcard_match("MyCustomId", "mycustomid"));
    // '*' matches any sequence.
    assert!(wildcard_match("*", "123456789"));
    assert!(wildcard_match("*", ""));
    assert!(wildcard_match("*", "*abc"));
    assert!(wildcard_match("123*", "123456789"));
    assert!(wildcard_match("123*", "123"));
    assert!(wildcard_match("12*", "12*9"));
    assert!(!wildcard_match("123*", "124456789"));
    assert!(wildcard_match("*789", "123456789"));
    assert!(wildcard_match("1*9", "123456789"));
    assert!(wildcard_match("1*4*9", "123456789"));
    assert!(!wildcard_match("1*4*9", "123456780"));
    assert!(wildcard_match("*456*", "123456789"));
    // '?' matches exactly one character.
    assert!(wildcard_match("12345678?", "123456789"));
    assert!(!wildcard_match("123456789?", "123456789"));
    assert!(wildcard_match("???456???", "123456789"));
    assert!(wildcard_match("1?3*7?9", "123456789"));
    // Whitespace around entries is ignored.
    assert!(wildcard_match(" 123456789 ", "123456789"));
}

#[test]
pub(super) fn test_decay_stale_failures() {
    let entry = |minute: i32| (minute, 1, 40);
    let keys = ["ip".to_string(), "p64".to_string(), "absent".to_string()];
    let mut m: HashMap<String, (i32, i32, i32)> = HashMap::new();
    m.insert("ip".to_string(), entry(100));
    m.insert("p64".to_string(), entry(160));
    m.insert("untouched".to_string(), entry(100));

    // Exactly at the window: forgotten. Still inside it: kept.
    decay_stale_failures(&mut m, &keys, 160, 60);
    assert!(!m.contains_key("ip"));
    assert!(m.contains_key("p64"));
    // Keys that were not passed in are never visited, absent ones are a no-op.
    assert!(m.contains_key("untouched"));

    // One minute short of the window keeps the entry.
    decay_stale_failures(&mut m, &keys, 219, 60);
    assert!(m.contains_key("p64"));
    decay_stale_failures(&mut m, &keys, 220, 60);
    assert!(!m.contains_key("p64"));

    // A clock that jumped backwards must not drop anything.
    m.insert("ip".to_string(), entry(500));
    decay_stale_failures(&mut m, &keys, 0, 60);
    assert!(m.contains_key("ip"));
}

#[test]
pub(super) fn test_clear_failures_drops_shared_prefixes() {
    // On IPv6 a whitelisted peer usually has no entry of its own, while the shared
    // prefixes that block it do. Clearing must not depend on the per-address entry.
    let mut m: HashMap<String, (i32, i32, i32)> = HashMap::new();
    m.insert("p64".to_string(), (100, 1, 55));
    m.insert("p56".to_string(), (100, 1, 75));
    m.insert("p48".to_string(), (100, 1, 95));
    m.insert("someone-else".to_string(), (100, 1, 95));
    let keys = ["ip", "p64", "p56", "p48"].map(|k| k.to_string());

    clear_failures(&mut m, &keys);

    for key in ["p64", "p56", "p48"] {
        assert!(!m.contains_key(key), "{key} should have been cleared");
    }
    // Keys belonging to other peers are left alone.
    assert!(m.contains_key("someone-else"));
}

#[test]
pub(super) fn test_id_whitelist_allows() {
    let list = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();

    // An empty whitelist allows everyone.
    assert!(id_whitelist_allows(&[], "123456789"));

    // Same server: the peer reports a bare id.
    assert!(id_whitelist_allows(&list(&["123456789"]), "123456789"));
    assert!(!id_whitelist_allows(&list(&["123456789"]), "987654321"));

    // Cross server: the peer appends its own server, which must not reject it.
    assert!(id_whitelist_allows(
        &list(&["123456789"]),
        "123456789@example.com:21116"
    ));
    // Cross server from web, whose server is a WebSocket URI.
    assert!(id_whitelist_allows(
        &list(&["123456789"]),
        "123456789@wss://example.com:21118/ws/id"
    ));
    // A different id is still rejected, suffix or not.
    assert!(!id_whitelist_allows(
        &list(&["123456789"]),
        "987654321@example.com:21116"
    ));

    // An entry pinned to one server keeps matching that exact form.
    assert!(id_whitelist_allows(
        &list(&["123456789@example.com:21116"]),
        "123456789@example.com:21116"
    ));
    assert!(!id_whitelist_allows(
        &list(&["123456789@example.com:21116"]),
        "123456789@other.com:21116"
    ));
    // ... and no longer matches the bare id, which is the point of pinning.
    assert!(!id_whitelist_allows(
        &list(&["123456789@example.com:21116"]),
        "123456789"
    ));

    // Wildcards keep working on both forms.
    assert!(id_whitelist_allows(&list(&["abc*"]), "abcdef"));
    assert!(id_whitelist_allows(
        &list(&["abc*"]),
        "abcdef@example.com:21116"
    ));
    assert!(id_whitelist_allows(
        &list(&["*"]),
        "123456789@example.com:21116"
    ));

    // Any entry of the list is enough.
    assert!(id_whitelist_allows(
        &list(&["111111111", "123456789", "222222222"]),
        "123456789@example.com:21116"
    ));
}
