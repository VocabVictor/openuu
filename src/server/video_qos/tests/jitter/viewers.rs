use super::*;

#[test]
pub(super) fn bitrate_timer_does_not_punish_unconfirmed_spikes_or_stale_averages() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    for delay in [800, 10, 350, 10, 350, 10].repeat(10) {
        qos.user_network_delay(1, delay);
        qos.adjust_ratio(false);
        assert_eq!(qos.fps(), FPS);
        assert_eq!(qos.ratio(), ratio);
    }
}

#[test]
pub(super) fn viewers_confirm_congestion_independently() {
    let mut qos = stable_qos();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    for id in [1, 2, 1, 2] {
        qos.user_network_delay(id, 400);
        assert_eq!(qos.fps(), FPS);
    }
    qos.user_network_delay(2, 10);
    qos.user_network_delay(1, 400);
    assert!(qos.fps() < FPS);
    qos.user_custom_fps(2, 12);
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), 12);
}

#[test]
pub(super) fn a_congested_viewer_does_not_lower_another_viewers_target() {
    let mut qos = stable_qos();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    assert_eq!(qos.fps(), FPS);
    // Viewer 1 congests; the stream follows the slowest viewer.
    for _ in 0..2 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 8);
    // Viewer 2 is fine and keeps its own target rather than inheriting viewer 1's.
    qos.user_network_delay(2, 10);
    assert_eq!(qos.users[&2].delay.fps, Some(FPS));
    // Once viewer 1 restores, the stream is back at once.
    for _ in 0..3 {
        qos.user_network_delay(1, 10);
    }
    assert_eq!(qos.fps(), FPS);
}

#[test]
pub(super) fn pending_probe_checks_do_not_count_as_fresh_bad_replies() {
    let mut qos = stable_qos();
    qos.user_network_delay(1, 400);
    for elapsed in [1000, 1500, 1900] {
        qos.user_delay_response_elapsed(1, elapsed);
        assert_eq!(qos.fps(), FPS);
    }
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 10);
    qos.user_network_delay(1, 400);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
}

#[test]
pub(super) fn recovery_continues_with_intermittent_jitter() {
    let mut qos = stable_qos();
    for _ in 0..3 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 5);
    for delay in [10, 350].repeat(30) {
        qos.user_network_delay(1, delay);
    }
    assert_eq!(qos.fps(), FPS);
}

#[test]
pub(super) fn custom_limit_of_one_viewer_does_not_lower_another_viewers_target() {
    let mut qos = stable_qos();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    qos.user_custom_fps(2, 12);
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), 12, "the stream follows the lowest limit");
    assert_eq!(
        qos.users[&1].delay.fps,
        Some(FPS),
        "viewer 1's own target is not a function of viewer 2's limit"
    );
    qos.on_connection_close(2);
    assert_eq!(
        qos.fps(),
        FPS,
        "the stream is back the moment the limit is gone"
    );
}

#[test]
pub(super) fn new_viewers_first_reply_does_not_bypass_bitrate_cooldown() {
    let mut qos = abr_session();
    for _ in 0..3 {
        qos.user_network_delay(1, 800);
    }
    // Viewer 1 is confirmed and its evidence was spent on a cut a moment ago.
    let ratio = qos.ratio();
    assert!(ratio < Quality::Balanced.ratio());
    qos.users.insert(2, UserData::default());
    qos.user_network_delay(2, 10);
    assert_eq!(
        qos.ratio(),
        ratio,
        "viewer 2's first reply must not spend viewer 1's evidence again inside the cooldown"
    );
}

#[test]
pub(super) fn new_viewer_does_not_inherit_another_viewers_congested_fps() {
    let mut qos = stable_qos();
    for _ in 0..2 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 8);
    qos.users.insert(2, UserData::default());
    qos.user_network_delay(2, 10);
    assert!(
        qos.users[&2].delay.fps >= Some(INIT_FPS),
        "a new viewer starts from INIT_FPS, not from the congested stream: {:?}",
        qos.users[&2].delay.fps
    );
}

#[test]
pub(super) fn unconfirmed_severe_viewer_does_not_amplify_another_viewers_confirmed_mild_congestion() {
    let mut qos = abr_session();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    // The first reply of a new viewer adjusts the ratio and restarts the cooldown.
    qos.advance_ms(4000);
    let ratio = qos.ratio();
    // Viewer 2: mild congestion, confirmed over three replies.  Viewer 1: one
    // severe spike, never confirmed.  Each viewer on its own calls for at most a
    // five percent step; together they must not turn into a halving.
    qos.user_network_delay(2, 200);
    qos.user_network_delay(1, 1200);
    qos.user_network_delay(2, 200);
    let after_two = qos.ratio();
    assert!(after_two < ratio, "viewer 2's second bad reply cuts");
    assert!(
        after_two >= ratio * 0.94,
        "viewer 2's own mild excess is a five percent step, not {after_two}"
    );
    qos.user_network_delay(2, 200);
    qos.advance_ms(4000);
    qos.update_display_data("test", 30);
    assert!(
        qos.ratio() >= after_two * 0.94,
        "viewer 1's severity must not be paired with viewer 2's confirmation: {}",
        qos.ratio()
    );
}

/// What `on_connection_open` inserts, without touching the config store.
pub(super) fn newcomer(qos: &VideoQoS) -> UserData {
    UserData {
        joined_at: Some(qos.now()),
        ..Default::default()
    }
}

#[test]
pub(super) fn closing_a_just_opened_viewer_does_not_throttle_existing_viewers() {
    let mut qos = stable_qos();
    assert_eq!(qos.fps(), FPS);
    qos.users.insert(2, newcomer(&qos));
    assert_eq!(
        qos.fps(),
        FPS,
        "nothing changes until the stream is re-aggregated"
    );
    qos.on_connection_close(2);
    assert_eq!(
        qos.fps(),
        FPS,
        "the guard leaves with the viewer that brought it"
    );
}

#[test]
pub(super) fn a_new_viewer_caps_the_stream_at_init_fps_for_a_second() {
    let mut qos = stable_qos();
    qos.users.insert(2, newcomer(&qos));
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), INIT_FPS);
    qos.advance_ms(1000);
    qos.user_network_delay(2, 10);
    qos.user_network_delay(1, 10);
    assert!(
        qos.fps() > INIT_FPS,
        "after a second the stream follows the viewers' own targets: {}",
        qos.fps()
    );
}

#[test]
pub(super) fn closing_the_latest_newcomer_keeps_an_earlier_newcomers_guard() {
    let mut qos = stable_qos();
    qos.users.insert(2, newcomer(&qos));
    qos.user_network_delay(2, 10);
    assert_eq!(qos.fps(), INIT_FPS);
    qos.advance_ms(100);
    qos.users.insert(3, newcomer(&qos));
    qos.advance_ms(100);
    qos.on_connection_close(3);
    assert_eq!(
        qos.fps(),
        INIT_FPS,
        "viewer 2 is still inside its own start-up window"
    );
}
