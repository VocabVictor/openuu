//! What the heartbeat collects, and when it does not bother.
use super::*;
use std::cell::Cell;

fn info(username: &str) -> Value {
    json!({ "username": username, "hostname": "vm" })
}

fn uploaded_at(at: Option<Instant>, username: &str) -> InfoUploaded {
    InfoUploaded {
        uploaded: true,
        url: "https://rs.example.com/api/heartbeat".to_owned(),
        last_uploaded: at,
        id: "123456789".to_owned(),
        username: Some(username.to_owned()),
    }
}

fn long_ago() -> Instant {
    Instant::now()
        .checked_sub(UPLOAD_SYSINFO_TIMEOUT + Duration::from_secs(1))
        .expect("an instant before the timeout")
}

/// Counts what the collection would have cost.
fn collected(state: &InfoUploaded) -> (Option<(Value, String)>, u32) {
    let calls = Cell::new(0);
    let out = state.sysinfo_to_upload(|| {
        calls.set(calls.get() + 1);
        info("alice")
    });
    (out, calls.get())
}

#[test]
fn a_device_that_has_never_uploaded_collects_and_uploads() {
    let state = InfoUploaded::default();
    let (out, calls) = collected(&state);
    assert_eq!(calls, 1);
    let (_, username) = out.expect("the first heartbeat uploads");
    assert_eq!(username, "alice");
}

#[test]
fn a_tick_that_could_not_upload_does_not_collect() {
    let state = uploaded_at(Some(Instant::now()), "alice");
    let (out, calls) = collected(&state);
    assert_eq!(calls, 0, "the system was asked about itself for nothing");
    assert!(out.is_none());
}

#[test]
fn nothing_is_uploaded_again_when_nothing_has_changed() {
    let state = uploaded_at(Some(long_ago()), "alice");
    let (out, calls) = collected(&state);
    assert_eq!(calls, 1, "due, so it has to look");
    assert!(out.is_none(), "same machine, same user");
}

#[test]
fn a_new_user_on_the_machine_is_uploaded_once_it_is_due() {
    let state = uploaded_at(Some(long_ago()), "bob");
    let (out, calls) = collected(&state);
    assert_eq!(calls, 1);
    assert_eq!(out.expect("the user changed").1, "alice");
}

/// The cost being removed: at one tick every three seconds, a device that uploaded a
/// moment ago used to collect twenty times a minute and now collects at most once every
/// two minutes.
#[test]
fn an_idle_minute_collects_nothing() {
    let state = uploaded_at(Some(Instant::now()), "alice");
    let mut calls = 0;
    for _ in 0..(60 / 3) {
        let (_, c) = collected(&state);
        calls += c;
    }
    assert_eq!(calls, 0);
}
