use super::*;

fn stable_qos() -> VideoQoS {
    let mut qos = VideoQoS::default();
    qos.advance_ms(2000);
    qos.users.insert(1, UserData::default());
    for _ in 0..12 {
        qos.user_network_delay(1, 10);
    }
    assert_eq!(qos.fps(), FPS);
    qos
}

#[test]
fn isolated_delay_spike_does_not_lower_fps() {
    let mut qos = stable_qos();
    for delay in [800, 10, 10, 10] {
        qos.user_network_delay(1, delay);
        assert_eq!(qos.fps(), FPS);
    }
}

#[test]
fn occasional_spikes_do_not_accumulate_congestion() {
    let mut qos = stable_qos();
    for delay in [800, 10, 10].repeat(20) {
        qos.user_network_delay(1, delay);
        assert_eq!(qos.fps(), FPS);
    }
}

#[test]
fn sustained_delay_reduces_fps_gradually() {
    let mut qos = stable_qos();
    for expected_fps in [30, 30, 24, 24, 24, 20] {
        qos.user_network_delay(1, 800);
        assert_eq!(qos.fps(), expected_fps);
    }
}

#[test]
fn delay_history_keeps_two_samples() {
    let mut delay = UserDelay::default();
    for sample in [1, 2, 3] {
        delay.add_delay(sample);
    }
    assert_eq!(delay.delay_history.len(), HISTORY_DELAY_LEN);
}

#[test]
fn response_timeout_halves_fps_for_each_second_outstanding() {
    let mut qos = stable_qos();
    for (elapsed, expected) in [(2001, 15), (3001, 7), (4001, 5), (5001, 5), (6001, 5)] {
        qos.user_delay_response_elapsed(1, elapsed);
        assert_eq!(qos.fps(), expected, "{elapsed} ms outstanding");
    }
}

#[test]
fn severe_delay_does_not_wait_for_another_reply() {
    let mut qos = stable_qos();
    qos.user_network_delay(1, 1200);
    assert_eq!(qos.fps(), 15);
}

#[test]
fn response_timeout_recovers_in_two_good_replies() {
    let mut qos = stable_qos();
    qos.user_delay_response_elapsed(1, 3000);
    assert_eq!(qos.fps(), 7);
    qos.user_network_delay(1, 3200);
    assert_eq!(
        qos.fps(),
        7,
        "the late reply belongs to the stall that was braked"
    );
    qos.user_delay_response_elapsed(1, 0);
    qos.user_network_delay(1, 10);
    assert_eq!(
        qos.fps(),
        8,
        "one good reply must not restore the full frame rate"
    );
    qos.user_network_delay(1, 10);
    assert_eq!(
        qos.fps(),
        FPS,
        "the second good reply restores the frame rate"
    );
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), FPS, "the third keeps the restored frame rate");
}

#[test]
fn restore_aims_lower_after_a_restore_that_congested() {
    let mut qos = stable_qos();
    for _ in 0..2 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 8);
    qos.user_network_delay(1, 10);
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), FPS);
    // The restored level congests at once, so the next restore aims lower.
    for _ in 0..3 {
        qos.user_network_delay(1, 400);
    }
    assert!(qos.fps() < FPS);
    qos.user_network_delay(1, 10);
    qos.user_network_delay(1, 10);
    assert!(
        qos.fps() < FPS,
        "no return to the level that failed: {}",
        qos.fps()
    );
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), FPS, "ordinary recovery can still reach the cap");
}

#[test]
fn custom_fps_limit_applies_during_delay_spike() {
    let mut qos = stable_qos();
    qos.user_custom_fps(1, 12);
    qos.user_network_delay(1, 800);
    assert_eq!(qos.fps(), 12);
}

mod adaptation;
mod baseline;
mod invariants;
mod jitter;
mod recovery;
mod robustness;
mod sim;
mod smoke;
mod startup;
