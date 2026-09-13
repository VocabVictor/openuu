//! How a session change reaches the service loop.
use super::service::wake_ipc_loop;
use crate::ipc;
use hbb_common::{timeout, tokio};
use std::time::{Duration, Instant};

#[tokio::test(flavor = "current_thread")]
async fn a_session_change_ends_the_loops_wait_at_once() {
    // A pipe of its own, so the test never touches the one a running service owns.
    let mut incoming = match ipc::new_listener("_wake_test").await {
        Ok(incoming) => incoming,
        // A machine that will not give us a pipe can tell us nothing about waking.
        Err(err) => return println!("skipped: {err}"),
    };
    let waker = std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(100));
        wake_ipc_loop("_wake_test");
    });

    let start = Instant::now();
    let woken = timeout(3_000, incoming.next()).await;
    waker.join().expect("the waking thread");
    assert!(woken.is_ok(), "the wait was not ended");
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "woken after {:?}",
        start.elapsed()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn without_a_session_change_the_loop_waits_out_its_backstop() {
    let mut incoming = match ipc::new_listener("_wake_test_idle").await {
        Ok(incoming) => incoming,
        Err(err) => return println!("skipped: {err}"),
    };
    let start = Instant::now();
    let woken = timeout(200, incoming.next()).await;
    assert!(woken.is_err(), "nothing connected, yet the wait ended");
    assert!(start.elapsed() >= Duration::from_millis(180));
}
