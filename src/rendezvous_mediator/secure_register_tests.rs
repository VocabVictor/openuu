use super::secure_register::*;
use super::tokio;

lazy_static::lazy_static! {
    /// The state under test is process-wide, because what it describes is: one
    /// flag and one set of hosts, shared by every mediator. These run one at a
    /// time rather than pretending otherwise with distinct host names, which
    /// would not separate the flag.
    static ref ONE_AT_A_TIME: super::Mutex<()> = Default::default();
}

#[test]
fn unset_is_not_turned_off() {
    assert!(matches!(Udp::of(""), Udp::Unset));
    assert!(matches!(Udp::of("N"), Udp::On));
    assert!(matches!(Udp::of("Y"), Udp::Off));
    assert!(matches!(Udp::of("y"), Udp::Unset));
}

#[tokio::test]
async fn a_session_that_registered_never_marks_the_host() {
    let _serial = ONE_AT_A_TIME.lock().await;
    forget().await;
    session_start();
    answered();
    // However it ended -- reset, timeout, restart -- the server does support
    // registering on TCP, so the next attempt has to use it again.
    assert!(!session_end("hbbs.example").await);
    assert!(!tcp_broken("hbbs.example").await);
}

#[tokio::test]
async fn silence_sends_the_next_attempt_to_udp() {
    let _serial = ONE_AT_A_TIME.lock().await;
    forget().await;
    session_start();
    assert!(session_end("hbbs.example").await);
    assert!(tcp_broken("hbbs.example").await);
    // Only that host: another deployment is judged on its own behaviour.
    assert!(!tcp_broken("other.example").await);
}

#[tokio::test]
async fn the_flag_does_not_leak_between_sessions() {
    let _serial = ONE_AT_A_TIME.lock().await;
    forget().await;
    session_start();
    assert!(session_end("silent.example").await);
    session_start();
    answered();
    assert!(!session_end("talkative.example").await);
    assert!(tcp_broken("silent.example").await);
}
