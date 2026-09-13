//! The encoder picked for `PreferCodec::Auto`, as a pure function of what
//! the peers can decode and what this machine can afford. Hardware H265 /
//! H264 come first; software AV1 only when the machine has a hardware
//! encoder to spare or more than four logical cores (perf-review P0-4:
//! software AV1 on a 2-vCPU host reached 10 fps at 77% CPU, VP9 is far
//! cheaper); otherwise VP9, and VP8 on a machine with at most 4 GiB.

use crate::CodecFormat;

/// Logical cores at or below which software AV1 is not chosen.
pub const AV1_MIN_CORES_EXCLUSIVE: usize = 4;
const VP8_MAX_MEMORY: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default)]
pub struct AutoCodecInputs {
    pub vp8_useable: bool,
    pub av1_useable: bool,
    pub h264_useable: bool,
    pub h265_useable: bool,
    /// A hardware (RAM or VRAM) encoder exists for some format.
    pub hw_encoder_available: bool,
    pub logical_cores: usize,
    pub total_memory: u64,
}

pub fn choose_auto_codec(i: &AutoCodecInputs) -> CodecFormat {
    if i.h265_useable {
        return CodecFormat::H265;
    }
    if i.h264_useable {
        return CodecFormat::H264;
    }
    let av1_affordable = i.hw_encoder_available || i.logical_cores > AV1_MIN_CORES_EXCLUSIVE;
    let mut codec = if i.av1_useable && av1_affordable {
        CodecFormat::AV1
    } else {
        CodecFormat::VP9
    };
    if i.vp8_useable && i.total_memory <= VP8_MAX_MEMORY {
        codec = CodecFormat::VP8;
    }
    codec
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn software(cores: usize, memory: u64) -> AutoCodecInputs {
        AutoCodecInputs {
            vp8_useable: true,
            av1_useable: true,
            h264_useable: false,
            h265_useable: false,
            hw_encoder_available: false,
            logical_cores: cores,
            total_memory: memory,
        }
    }

    #[test]
    fn few_cores_without_hardware_pick_vp9_over_av1() {
        assert_eq!(choose_auto_codec(&software(2, 16 * GIB)), CodecFormat::VP9);
        assert_eq!(choose_auto_codec(&software(4, 16 * GIB)), CodecFormat::VP9);
    }

    #[test]
    fn enough_cores_keep_av1() {
        assert_eq!(choose_auto_codec(&software(5, 16 * GIB)), CodecFormat::AV1);
        assert_eq!(choose_auto_codec(&software(20, 16 * GIB)), CodecFormat::AV1);
    }

    #[test]
    fn a_hardware_encoder_keeps_av1_on_few_cores() {
        let mut i = software(2, 16 * GIB);
        i.hw_encoder_available = true;
        assert_eq!(choose_auto_codec(&i), CodecFormat::AV1);
    }

    #[test]
    fn av1_not_decodable_falls_back_to_vp9() {
        let mut i = software(8, 16 * GIB);
        i.av1_useable = false;
        assert_eq!(choose_auto_codec(&i), CodecFormat::VP9);
    }

    #[test]
    fn low_memory_prefers_vp8_regardless_of_cores() {
        assert_eq!(choose_auto_codec(&software(2, 4 * GIB)), CodecFormat::VP8);
        assert_eq!(choose_auto_codec(&software(16, 2 * GIB)), CodecFormat::VP8);
        let mut i = software(2, 4 * GIB);
        i.vp8_useable = false;
        assert_eq!(choose_auto_codec(&i), CodecFormat::VP9);
    }

    #[test]
    fn hardware_h26x_wins_whatever_the_cores() {
        let mut i = software(2, 4 * GIB);
        i.h264_useable = true;
        i.hw_encoder_available = true;
        assert_eq!(choose_auto_codec(&i), CodecFormat::H264);
        i.h265_useable = true;
        assert_eq!(choose_auto_codec(&i), CodecFormat::H265);
    }
}
