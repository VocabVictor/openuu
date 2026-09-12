use super::*;

pub(super) fn abr_session_from_scratch() -> VideoQoS {
    let mut qos = VideoQoS::default();
    qos.advance_ms(2000);
    qos.users.insert(1, UserData::default());
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    qos
}

/// The video loop reports the encoder's bitrate as soon as it applies a new ratio.
pub(super) fn sync_bitrate(qos: &mut VideoQoS) {
    let target = qos.latest_quality().ratio();
    let ratio = qos.ratio();
    qos.store_bitrate((4000.0 * ratio / target) as u32);
}

/// One second of wall clock, one probe reply, one display update: what a
/// connection does every second.
pub(super) fn second(qos: &mut VideoQoS, delay: u32, encoded: usize) {
    qos.advance_ms(1000);
    sync_bitrate(qos);
    qos.user_network_delay(1, delay);
    sync_bitrate(qos);
    qos.update_display_data("test", encoded);
    sync_bitrate(qos);
}

#[test]
pub(super) fn stable_high_rtt_restores_bitrate() {
    for rtt in [180, 300] {
        let mut qos = abr_session_from_scratch();
        let target = qos.latest_quality().ratio();
        for _ in 0..120 {
            second(&mut qos, rtt, 30);
        }
        assert_eq!(qos.fps(), FPS, "rtt {rtt}");
        assert!(
            qos.ratio() >= target * 0.99,
            "rtt {rtt}: ratio {}",
            qos.ratio()
        );
    }
}

#[test]
pub(super) fn congestion_bitrate_reduction_resets_dynamic_screen_window() {
    // A static screen encodes about one frame per second.  While the congestion path
    // adjusts the ratio at every cooldown, the periodic branch never runs, so the
    // encode counter must not keep accumulating across the whole episode.
    let mut qos = abr_session();
    for _ in 0..10 {
        qos.advance_ms(4000);
        qos.user_network_delay(1, 10);
        qos.user_network_delay(1, 400);
        qos.user_network_delay(1, 400);
        qos.update_display_data("test", 1);
    }
    for _ in 0..12 {
        qos.advance_ms(1000);
        qos.user_network_delay(1, 10);
    }
    let ratio = qos.ratio();
    qos.advance_ms(4000);
    qos.update_display_data("test", 1);
    assert!(
        qos.ratio() <= ratio,
        "a static screen must not look dynamic after congestion"
    );
}

#[test]
pub(super) fn a_single_stall_does_not_cut_bitrate() {
    // One probe out for 2.5 s, then its late reply: jitter, not congestion.
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_delay_response_elapsed(1, 2500);
    qos.advance_ms(1000);
    qos.update_display_data("test", 30);
    assert_eq!(qos.ratio(), ratio, "the timeout tick alone");
    qos.user_network_delay(1, 2600);
    assert_eq!(qos.ratio(), ratio, "the late reply alone");
    qos.user_network_delay(1, 400);
    assert!(qos.ratio() < ratio, "a second bad reply confirms");
}

#[test]
pub(super) fn a_stall_beyond_three_seconds_cuts_bitrate() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_delay_response_elapsed(1, 2001);
    qos.update_display_data("test", 30);
    assert_eq!(qos.ratio(), ratio);
    qos.advance_ms(1000);
    qos.user_delay_response_elapsed(1, 3001);
    qos.update_display_data("test", 30);
    assert!(qos.ratio() < ratio, "still out at the next tick");
}

#[test]
pub(super) fn stable_high_rtt_does_not_dip_at_start() {
    for rtt in [180, 300] {
        let mut qos = super::super::smoke::session(30, Quality::Balanced);
        for _ in 0..20 {
            qos.advance_ms(1000);
            qos.user_network_delay(1, rtt);
            assert!(qos.fps() >= INIT_FPS, "rtt {rtt}: {}", qos.fps());
        }
        assert_eq!(qos.fps(), FPS, "rtt {rtt}");
    }
}

#[test]
pub(super) fn confirmed_severe_congestion_halves_bitrate() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_network_delay(1, 800);
    qos.user_network_delay(1, 800); // two bad replies: an ordinary step
    let after_first = qos.ratio();
    assert!(
        after_first < ratio && after_first > ratio * 0.75,
        "{after_first}"
    );
    qos.user_network_delay(1, 800); // three: confirmed
    qos.advance_ms(4000);
    qos.update_display_data("test", 30);
    assert!(qos.ratio() <= after_first * 0.55, "{}", qos.ratio());
}

#[test]
pub(super) fn fps_holds_its_floor_while_bitrate_can_still_drop() {
    // With a bitrate-targeted encoder fewer frames do not mean fewer bytes, so the
    // bitrate comes down first and the frame rate keeps its floor meanwhile.
    let mut qos = abr_session();
    let mut reached_floor = false;
    // Keep the queue growing, rather than presenting a stable new path delay.
    let mut delay = 400;
    for _ in 0..60 {
        qos.advance_ms(3000);
        second(&mut qos, delay, 30);
        delay += 10;
        if qos.ratio() > 0.17 {
            assert!(
                qos.fps() >= 10,
                "fps {} at ratio {}",
                qos.fps(),
                qos.ratio()
            );
        } else {
            reached_floor = true;
            break;
        }
    }
    assert!(
        reached_floor,
        "bitrate must reach its floor: {}",
        qos.ratio()
    );
    for _ in 0..24 {
        qos.user_network_delay(1, delay);
        delay += 10;
    }
    assert!(
        qos.fps() < 10,
        "an exhausted bitrate frees the frame rate: {}",
        qos.fps()
    );
}
