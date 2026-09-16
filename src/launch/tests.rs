use super::*;
#[test]
fn rejects_oversized_request() { assert!(handle(&"x".repeat(32769)).contains("too large")); }
#[test]
fn rejects_unknown_fields() { assert!(handle(r#"{"request_id":"1","operation":"list","shell":"bad"}"#).contains("unknown field")); }
#[test]
fn denial_preserves_request_identity() {
    let reply: serde_json::Value = serde_json::from_str(&denied(r#"{"request_id":"abc","operation":"launch"}"#)).expect("JSON");
    assert_eq!(reply["request_id"], "abc");
    assert!(reply.get("data").is_none());
    assert!(reply.get("error").is_some());
}
#[test]
fn rejects_null_in_arguments_before_launch() {
    let reply = handle(r#"{"request_id":"x","operation":"launch","arguments":["\u0000"]}"#);
    assert!(reply.contains("Invalid request"));
}
#[cfg(target_os = "windows")]
#[test]
fn quotes_windows_arguments() {
    assert_eq!(quote_windows("a b"), "\"a b\"");
    assert_eq!(quote_windows("a\"b"), "\"a\\\"b\"");
    assert_eq!(quote_windows("C:\\x\\"), "\"C:\\x\\\\\"");
}
