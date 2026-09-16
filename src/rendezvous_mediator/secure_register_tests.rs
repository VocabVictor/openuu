use super::secure_register::*;
use super::tokio;

lazy_static::lazy_static! {
    /// The state under test is process-wide, because the thing it describes is: one
    /// registration flag and one set of hosts, shared by every mediator. Tests that touch
    /// it run one at a time rather than pretending otherwise with distinct host names,
    /// which would not separate the flag.
    static ref ONE_AT_A_TIME: super::Mutex<()> = Default::default();
}

/// The three states exist so that "never touched" and "turned off" are not the same
/// answer; a bool here would make the fallback either unreachable or unavoidable.
#[test]
fn an_unset_switch_is_not_the_same_as_one_turned_off() {
    assert!(matches!(UdpPreference::of(""), UdpPreference::Default));
    assert!(matches!(UdpPreference::of("N"), UdpPreference::UdpOnly));
    assert!(matches!(UdpPreference::of("Y"), UdpPreference::TcpOnly));
    // Anything else is "not configured", the same as empty.
    assert!(matches!(UdpPreference::of("y"), UdpPreference::Default));
}

#[tokio::test]
async fn a_session_that_registered_never_marks_the_host() {
    let _serial = ONE_AT_A_TIME.lock().await;
    forget_hosts().await;
    begin_tcp_session();
    registration_answered();
    // However this session ended -- reset, timeout, restart -- the server does support
    // registering on TCP, so the next attempt has to use it again.
    assert!(!tcp_session_ended("hbbs.example").await);
    assert!(!tcp_register_known_broken("hbbs.example").await);
}

#[tokio::test]
async fn a_session_with_no_answer_sends_the_next_attempt_to_udp() {
    let _serial = ONE_AT_A_TIME.lock().await;
    forget_hosts().await;
    begin_tcp_session();
    assert!(tcp_session_ended("hbbs.example").await);
    assert!(tcp_register_known_broken("hbbs.example").await);
    // Only that host: a second deployment is judged on its own behaviour.
    assert!(!tcp_register_known_broken("other.example").await);
}

#[tokio::test]
async fn one_host_failing_does_not_answer_for_the_next_session_of_another() {
    let _serial = ONE_AT_A_TIME.lock().await;
    forget_hosts().await;
    begin_tcp_session();
    assert!(tcp_session_ended("silent.example").await);
    // A new session starts clean, so the mark cannot leak through the flag.
    begin_tcp_session();
    registration_answered();
    assert!(!tcp_session_ended("talkative.example").await);
    assert!(tcp_register_known_broken("silent.example").await);
}
