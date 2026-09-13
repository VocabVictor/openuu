use super::*;
use super::super::test_support::next_message;
use hbb_common::config::OVERWRITE_SETTINGS;

/// Password-only approval with a permanent password, no session token on the
/// controlled side. Options are overridden in memory so nothing is written to
/// the config file on disk.
fn use_password_only_options() {
    let mut settings = OVERWRITE_SETTINGS.write().unwrap();
    settings.insert("approve-mode".to_owned(), "password".to_owned());
    settings.insert(
        "verification-method".to_owned(),
        "use-permanent-password".to_owned(),
    );
    settings.insert(keys::OPTION_ID_WHITELIST.to_owned(), "".to_owned());
    settings.insert("openuu-account-token".to_owned(), "".to_owned());
}

fn login_request(conn: &Connection, password: &str) -> Message {
    let mut lr = LoginRequest::new();
    lr.username = Config::get_id();
    lr.my_id = "test-controller".to_owned();
    lr.my_name = "test".to_owned();
    lr.my_platform = "Windows".to_owned();
    lr.version = VERSION.to_owned();
    lr.password = conn.hashed_login_password(password).into();
    let mut msg = Message::new();
    msg.set_login_request(lr);
    msg
}

fn login_error(msg: &Message) -> String {
    match &msg.union {
        Some(message::Union::LoginResponse(res)) => match &res.union {
            Some(login_response::Union::Error(err)) => err.clone(),
            other => panic!("expected LoginResponse.error, got {:?}", other),
        },
        other => panic!("expected LoginResponse, got {:?}", other),
    }
}

#[tokio::test]
async fn login_request_without_session_token_reaches_password_check() {
    use_password_only_options();
    assert!(crate::account::session_token().is_empty());
    let (mut conn, mut controller) = Connection::for_test(9001).await;

    let keep_open = conn.on_message(login_request(&conn, "not-the-password")).await;

    assert!(keep_open, "a failed password keeps the connection for a retry");
    assert!(!conn.authorized);
    assert_eq!(conn.lr.my_id, "test-controller");
    let err = login_error(&next_message(&mut controller).await);
    assert_ne!(err, "OpenUU login required on the controlled device");
    assert_eq!(err, crate::client::LOGIN_MSG_PASSWORD_WRONG);
}

#[tokio::test]
async fn wrong_password_returns_login_error() {
    use_password_only_options();
    let (mut conn, mut controller) = Connection::for_test(9002).await;

    assert!(conn.on_message(login_request(&conn, "wrong")).await);

    let err = login_error(&next_message(&mut controller).await);
    assert_eq!(err, crate::client::LOGIN_MSG_PASSWORD_WRONG);
    assert!(!conn.authorized);
    assert_eq!(conn.conn_audit_primary_auth, ConnAuditPrimaryAuth::None);
}
