use super::{race_transports_prefer_webrtc, request_allows_tcp_punch};
use hbb_common::{
    anyhow::anyhow,
    futures::future::{BoxFuture, FutureExt},
    tokio,
    tokio::time::{sleep, Duration, Instant},
    ResultType,
};

fn ok_after(ms: u64, tag: &'static str) -> BoxFuture<'static, ResultType<&'static str>> {
    async move {
        sleep(Duration::from_millis(ms)).await;
        Ok(tag)
    }
    .boxed()
}

fn err_after(ms: u64, what: &'static str) -> BoxFuture<'static, ResultType<&'static str>> {
    async move {
        sleep(Duration::from_millis(ms)).await;
        Err(anyhow!(what))
    }
    .boxed()
}

const NOT_P2P: fn(&&'static str) -> bool = |_| false;

#[test]
fn webrtc_request_never_reuses_its_signaling_socket_for_tcp_punch() {
    assert!(request_allows_tcp_punch(""));
    assert!(!request_allows_tcp_punch("webrtc://offer"));
}

// A direct result must win even when it lands first — the LAN ordering, where ICE beats the
// relay's TCP connect. Parking it as if it were a relay commits the relay on arrival.
#[tokio::test]
async fn direct_result_wins_even_when_it_arrives_first() {
    let got = race_transports_prefer_webrtc(
        ok_after(10, "direct"),
        vec![ok_after(120, "relay")],
        60_000,
        |result| *result == "direct",
    )
    .await
    .unwrap();
    assert_eq!(got, "direct");
}

#[tokio::test]
async fn webrtc_preferred_over_faster_relay_within_window() {
    let got = race_transports_prefer_webrtc(
        ok_after(120, "webrtc"),
        vec![ok_after(10, "relay")],
        60_000,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "webrtc");
}

// The preferred branch runs a whole punch attempt, so it can itself end in a relay. That must
// not preempt a direct punch still in flight on the fallback branch.
#[tokio::test]
async fn relay_from_preferred_branch_does_not_preempt_a_direct_fallback() {
    let got = race_transports_prefer_webrtc(
        ok_after(10, "preferred-relay"),
        vec![ok_after(120, "direct")],
        60_000,
        |tag| *tag == "direct",
    )
    .await
    .unwrap();
    assert_eq!(got, "direct");
}

// ...but it is still committed once nothing direct can arrive.
#[tokio::test]
async fn relay_from_preferred_branch_committed_when_fallback_fails() {
    let got = race_transports_prefer_webrtc(
        ok_after(10, "preferred-relay"),
        vec![err_after(50, "punch failed")],
        60_000,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "preferred-relay");
}

#[tokio::test]
async fn relay_from_preferred_branch_committed_when_window_expires() {
    let got = race_transports_prefer_webrtc(
        ok_after(10, "preferred-relay"),
        vec![ok_after(60_000, "too-slow")],
        100,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "preferred-relay");
}

#[tokio::test]
async fn preference_window_starts_when_relay_is_ready() {
    let got = race_transports_prefer_webrtc(
        ok_after(350, "webrtc"),
        vec![ok_after(250, "relay")],
        200,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "webrtc");
}

#[tokio::test]
async fn unfinished_ipv6_still_beats_held_relay() {
    let start = Instant::now();
    let got = race_transports_prefer_webrtc(
        ok_after(60_000, "webrtc"),
        vec![ok_after(10, "relay"), ok_after(100, "ipv6")],
        1_000,
        |t| *t == "ipv6",
    )
    .await
    .unwrap();
    assert_eq!(got, "ipv6");
    assert!(start.elapsed() < Duration::from_secs(5));
}

// WebRTC fails first (others still racing), then a relay arrives with a direct attempt still
// unfinished behind it. The relay must not be committed while that direct attempt can win.
#[tokio::test]
async fn relay_does_not_preempt_unfinished_direct_after_webrtc_fails() {
    let got = race_transports_prefer_webrtc(
        err_after(5, "webrtc dead"),
        vec![ok_after(10, "relay"), ok_after(100, "ipv6")],
        60_000,
        |t| *t == "ipv6",
    )
    .await
    .unwrap();
    assert_eq!(got, "ipv6");
}

// A relay is held with a direct attempt racing behind it, then WebRTC fails. The held relay
// must not be committed while the direct attempt is still in flight.
#[tokio::test]
async fn held_relay_waits_for_racing_direct_when_webrtc_fails() {
    let got = race_transports_prefer_webrtc(
        err_after(50, "webrtc dead"),
        vec![ok_after(10, "relay"), ok_after(100, "ipv6")],
        60_000,
        |t| *t == "ipv6",
    )
    .await
    .unwrap();
    assert_eq!(got, "ipv6");
}

// Both attempts error, but a relay was parked before they did — it is the outcome, not a
// composed error.
#[tokio::test]
async fn held_relay_survives_both_errors() {
    let got = race_transports_prefer_webrtc(
        err_after(50, "webrtc dead"),
        vec![ok_after(10, "relay"), err_after(100, "ipv6 dead")],
        60_000,
        |t| *t == "ipv6",
    )
    .await
    .unwrap();
    assert_eq!(got, "relay");
}

#[tokio::test]
async fn held_relay_committed_when_window_expires() {
    let start = Instant::now();
    let got = race_transports_prefer_webrtc(
        ok_after(60_000, "webrtc"),
        vec![ok_after(10, "relay")],
        150,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "relay");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn held_relay_committed_when_webrtc_fails() {
    let start = Instant::now();
    let got = race_transports_prefer_webrtc(
        err_after(50, "webrtc dead"),
        vec![ok_after(10, "relay")],
        60_000,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "relay");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn relay_committed_directly_after_webrtc_failed() {
    let got = race_transports_prefer_webrtc(
        err_after(5, "webrtc dead"),
        vec![ok_after(100, "relay")],
        60_000,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "relay");
}

#[tokio::test]
async fn webrtc_still_wins_past_window_when_relay_dead() {
    let got = race_transports_prefer_webrtc(
        ok_after(300, "webrtc"),
        vec![err_after(10, "relay dead")],
        50,
        NOT_P2P,
    )
    .await
    .unwrap();
    assert_eq!(got, "webrtc");
}

#[tokio::test]
async fn p2p_transport_committed_immediately() {
    let start = Instant::now();
    let got = race_transports_prefer_webrtc(
        ok_after(60_000, "webrtc"),
        vec![ok_after(10, "ipv6")],
        60_000,
        |t| *t == "ipv6",
    )
    .await
    .unwrap();
    assert_eq!(got, "ipv6");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn both_failing_compose_error() {
    let err = race_transports_prefer_webrtc(
        err_after(10, "webrtc dead"),
        vec![err_after(20, "relay dead")],
        1_000,
        NOT_P2P,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("webrtc dead") && err.contains("relay dead"), "{}", err);
}
