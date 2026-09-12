use super::*;

pub(super) fn abr_session() -> VideoQoS {
    let mut qos = stable_qos();
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    // Linux skips the first-reply adjustment; exercise it on every platform.
    qos.first_reply_adjusts_ratio = true;
    qos.advance_ms(4000);
    qos
}

#[test]
pub(super) fn bitrate_reduction_precedes_ordinary_fps_reduction() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    assert_eq!(qos.ratio(), ratio);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    assert!(qos.ratio() < ratio);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 400);
    assert!(qos.fps() < FPS);
}

#[test]
pub(super) fn bitrate_cooldown_defers_ordinary_fps_reduction() {
    let mut qos = abr_session();
    qos.adjust_ratio_instant = qos.now();
    let ratio = qos.ratio();
    for _ in 0..3 {
        qos.user_network_delay(1, 400);
        assert_eq!(qos.fps(), FPS);
        assert_eq!(qos.ratio(), ratio);
    }
    qos.advance_ms(4000);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    assert!(qos.ratio() < ratio);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 400);
    assert!(qos.fps() < FPS);
}

#[test]
pub(super) fn unavailable_abr_or_minimum_bitrate_does_not_prevent_fps_reduction() {
    for mode in ["disabled", "unsupported", "minimum"] {
        let mut qos = abr_session();
        match mode {
            "disabled" => qos.abr_config = false,
            "unsupported" => qos.set_support_changing_quality("test", false),
            "minimum" => qos.ratio = BR_MIN_HIGH_RESOLUTION,
            _ => unreachable!(),
        }
        for _ in 0..3 {
            qos.user_network_delay(1, 400);
        }
        assert!(qos.fps() < FPS, "{mode}");
    }
}

#[test]
pub(super) fn severe_delay_and_timeout_bypass_bitrate_cooldown() {
    let mut qos = abr_session();
    qos.adjust_ratio_instant = qos.now();
    qos.user_network_delay(1, 1200);
    assert_eq!(qos.fps(), 15);
    qos.user_delay_response_elapsed(1, 2500);
    assert_eq!(qos.fps(), 7);
}

#[test]
pub(super) fn minimum_bitrate_during_cooldown_does_not_block_fps_reduction() {
    // ABR on, ratio at its floor, a good reply cleared the post-reduction counter,
    // and the adjustment cooldown has just restarted: bitrate cannot help here.
    let mut qos = abr_session();
    qos.ratio = BR_MIN_HIGH_RESOLUTION;
    qos.user_network_delay(1, 10);
    qos.adjust_ratio_instant = qos.now();
    for _ in 0..3 {
        qos.user_network_delay(1, 400);
    }
    assert!(qos.fps() < FPS);
}
