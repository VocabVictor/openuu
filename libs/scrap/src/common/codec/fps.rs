//! Frame-rate bookkeeping for encoders whose rate control assumes a fixed frame rate.
//!
//! A hardware rate controller spends `kbs / fps` bits per frame for the fps it was created
//! with. When QoS paces the capture loop below that fps, the stream carries proportionally
//! less than the target; the wire rate follows the target again only if either the encoder
//! learns the new fps or the target is scaled up by the same factor.

/// The target to hand an encoder that still assumes `encoder_fps` when only `actual_fps`
/// frames a second reach it. Unchanged when the encoder gets at least its assumed fps.
pub fn fps_compensated_kbs(kbs: u32, encoder_fps: u32, actual_fps: u32) -> u32 {
    if actual_fps == 0 || encoder_fps == 0 || actual_fps >= encoder_fps {
        return kbs;
    }
    (kbs as u64 * encoder_fps as u64 / actual_fps as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slower_capture_scales_the_target_up_by_the_fps_ratio() {
        assert_eq!(fps_compensated_kbs(3000, 30, 10), 9000);
        assert_eq!(fps_compensated_kbs(3000, 30, 15), 6000);
        assert_eq!(fps_compensated_kbs(3000, 30, 5), 18000);
    }

    #[test]
    fn the_assumed_fps_or_faster_leaves_the_target_alone() {
        assert_eq!(fps_compensated_kbs(3000, 30, 30), 3000);
        assert_eq!(fps_compensated_kbs(3000, 30, 60), 3000);
    }

    #[test]
    fn zero_rates_never_divide() {
        assert_eq!(fps_compensated_kbs(3000, 30, 0), 3000);
        assert_eq!(fps_compensated_kbs(3000, 0, 10), 3000);
    }
}
