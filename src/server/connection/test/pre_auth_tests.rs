use super::*;
use super::super::test_support::{next_message, try_next_message};

fn test_delay(from_client: bool, last_delay: u32) -> Message {
    let mut msg = Message::new();
    msg.set_test_delay(TestDelay {
        from_client,
        last_delay,
        ..Default::default()
    });
    msg
}

fn auth_2fa(code: &str) -> Message {
    let mut msg = Message::new();
    msg.set_auth_2fa(Auth2FA {
        code: code.to_owned(),
        ..Default::default()
    });
    msg
}

fn totp_for_test() -> totp_rs::TOTP {
    totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        b"0123456789abcdef0123".to_vec(),
        None,
        "test".to_owned(),
    )
    .expect("totp")
}

#[tokio::test]
async fn test_delay_from_client_is_echoed_on_the_message_channel() {
    let (mut conn, _controller, mut rx) = Connection::for_test_with_sender(9101).await;

    assert!(conn.on_message(test_delay(true, 42)).await);

    let (_, echoed) = rx.try_recv().expect("echo queued for the message loop");
    match &echoed.union {
        Some(message::Union::TestDelay(t)) => {
            assert!(t.from_client);
            assert_eq!(t.last_delay, 42);
        }
        other => panic!("expected TestDelay, got {:?}", other),
    }
}

#[tokio::test]
async fn test_delay_reply_records_the_network_delay() {
    let (mut conn, _controller, mut rx) = Connection::for_test_with_sender(9102).await;
    conn.last_test_delay = Some(Instant::now() - Duration::from_millis(20));
    conn.network_delay = 0;

    assert!(conn.on_message(test_delay(false, 0)).await);

    assert!(conn.last_test_delay.is_none());
    assert!(conn.network_delay >= 20, "delay {}", conn.network_delay);
    assert!(rx.try_recv().is_err(), "a reply is not echoed");
}

#[tokio::test]
async fn auth_2fa_is_ignored_unless_awaited() {
    let (mut conn, mut controller) = Connection::for_test(9103).await;
    conn.require_2fa = Some(totp_for_test());
    conn.awaiting_2fa = false;

    assert!(conn.on_message(auth_2fa("123456")).await);

    assert!(conn.require_2fa.is_some());
    assert!(try_next_message(&mut controller, 200).await.is_none());
}

#[tokio::test]
async fn auth_2fa_wrong_code_answers_login_error() {
    let (mut conn, mut controller) = Connection::for_test(9104).await;
    conn.require_2fa = Some(totp_for_test());
    conn.awaiting_2fa = true;

    assert!(conn.on_message(auth_2fa("not-a-code")).await);

    let reply = next_message(&mut controller).await;
    match &reply.union {
        Some(message::Union::LoginResponse(res)) => match &res.union {
            Some(login_response::Union::Error(err)) => {
                assert_eq!(err, crate::client::LOGIN_MSG_2FA_WRONG)
            }
            other => panic!("expected LoginResponse.error, got {:?}", other),
        },
        other => panic!("expected LoginResponse, got {:?}", other),
    }
    assert!(conn.require_2fa.is_some(), "the TOTP stays armed after a wrong code");
    assert!(!conn.authorized);
}
