// https://webrtc.googlesource.com/src/+/refs/heads/main/modules/video_coding/codecs/av1/libaom_av1_encoder.cc
use super::*;

const kUsageProfile: u32 = AOM_USAGE_REALTIME;
const kBitDepth: u32 = 8;
const kLagInFrames: u32 = 0; // No look ahead.
pub(super) const kTimeBaseDen: i64 = 1000;

// Only positive speeds, range for real-time coding currently is: 6 - 8.
// Lower means slower/better quality, higher means fastest/lower quality.
fn get_cpu_speed(width: u32, height: u32) -> u32 {
    // aux_config_ = nullptr, kComplexityHigh
    if width * height <= 320 * 180 {
        8
    } else if width * height <= 640 * 360 {
        9
    } else {
        10
    }
}

fn tile_log2(threads: u32) -> std::os::raw::c_uint {
    (threads as f64).log2().ceil() as _
}

fn get_super_block_size(width: u32, height: u32, threads: u32) -> aom_superblock_size_t {
    use aom_superblock_size::*;
    let resolution = width * height;
    if threads >= 4 && resolution >= 960 * 540 && resolution < 1920 * 1080 {
        AOM_SUPERBLOCK_SIZE_64X64
    } else {
        AOM_SUPERBLOCK_SIZE_DYNAMIC
    }
}

pub fn enc_cfg(
    i: *const aom_codec_iface,
    cfg: AomEncoderConfig,
    i444: bool,
) -> ResultType<aom_codec_enc_cfg> {
    let mut c = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
    call_aom!(aom_codec_enc_config_default(i, &mut c, kUsageProfile));

    // Overwrite default config with input encoder settings & RTC-relevant values.
    c.g_w = cfg.width;
    c.g_h = cfg.height;
    c.g_threads = codec_thread_num(64) as _;
    c.g_timebase.num = 1;
    c.g_timebase.den = kTimeBaseDen as _;
    c.g_input_bit_depth = kBitDepth;
    if let Some(keyframe_interval) = cfg.keyframe_interval {
        c.kf_min_dist = 0;
        c.kf_max_dist = keyframe_interval as _;
    } else {
        c.kf_mode = aom_kf_mode::AOM_KF_DISABLED;
    }
    let (q_min, q_max) = AomEncoder::calc_q_values(cfg.quality);
    c.rc_min_quantizer = q_min;
    c.rc_max_quantizer = q_max;
    c.rc_target_bitrate = AomEncoder::bitrate(cfg.width as _, cfg.height as _, cfg.quality);
    c.rc_undershoot_pct = 50;
    c.rc_overshoot_pct = 50;
    c.rc_buf_initial_sz = 600;
    c.rc_buf_optimal_sz = 600;
    c.rc_buf_sz = 1000;
    c.g_usage = kUsageProfile;
    c.g_error_resilient = 0;
    // Low-latency settings.
    c.rc_end_usage = aom_rc_mode::AOM_CBR; // Constant Bit Rate (CBR) mode
    c.g_pass = aom_enc_pass::AOM_RC_ONE_PASS; // One-pass rate control
    c.g_lag_in_frames = kLagInFrames; // No look ahead when lag equals 0.

    // https://aomedia.googlesource.com/aom/+/refs/tags/v3.6.0/av1/common/enums.h#82
    c.g_profile = if i444 { 1 } else { 0 };

    Ok(c)
}

pub fn set_controls(ctx: *mut aom_codec_ctx_t, cfg: &aom_codec_enc_cfg) -> ResultType<()> {
    use aom_tune_content::*;
    use aome_enc_control_id::*;
    macro_rules! call_ctl {
        ($ctx:expr, $av1e:expr, $arg:expr) => {{
            call_aom_allow_err!(aom_codec_control($ctx, $av1e as i32, $arg));
        }};
    }

    call_ctl!(ctx, AOME_SET_CPUUSED, get_cpu_speed(cfg.g_w, cfg.g_h));
    call_ctl!(ctx, AV1E_SET_ENABLE_CDEF, 1);
    call_ctl!(ctx, AV1E_SET_ENABLE_TPL_MODEL, 0);
    call_ctl!(ctx, AV1E_SET_DELTAQ_MODE, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_ORDER_HINT, 0);
    call_ctl!(ctx, AV1E_SET_AQ_MODE, 3);
    call_ctl!(ctx, AOME_SET_MAX_INTRA_BITRATE_PCT, 300);
    call_ctl!(ctx, AV1E_SET_COEFF_COST_UPD_FREQ, 3);
    call_ctl!(ctx, AV1E_SET_MODE_COST_UPD_FREQ, 3);
    call_ctl!(ctx, AV1E_SET_MV_COST_UPD_FREQ, 3);
    // kScreensharing
    call_ctl!(ctx, AV1E_SET_TUNE_CONTENT, AOM_CONTENT_SCREEN);
    call_ctl!(ctx, AV1E_SET_ENABLE_PALETTE, 1);
    let tile_set = if cfg.g_threads == 4 && cfg.g_w == 640 && (cfg.g_h == 360 || cfg.g_h == 480)
    {
        AV1E_SET_TILE_ROWS
    } else {
        AV1E_SET_TILE_COLUMNS
    };
    call_ctl!(ctx, tile_set, tile_log2(cfg.g_threads));
    call_ctl!(ctx, AV1E_SET_ROW_MT, 1);
    call_ctl!(ctx, AV1E_SET_ENABLE_OBMC, 0);
    call_ctl!(ctx, AV1E_SET_NOISE_SENSITIVITY, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_WARPED_MOTION, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_GLOBAL_MOTION, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_REF_FRAME_MVS, 0);
    call_ctl!(
        ctx,
        AV1E_SET_SUPERBLOCK_SIZE,
        get_super_block_size(cfg.g_w, cfg.g_h, cfg.g_threads)
    );
    call_ctl!(ctx, AV1E_SET_ENABLE_CFL_INTRA, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_SMOOTH_INTRA, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_ANGLE_DELTA, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_FILTER_INTRA, 0);
    call_ctl!(ctx, AV1E_SET_INTRA_DEFAULT_TX_ONLY, 1);
    call_ctl!(ctx, AV1E_SET_DISABLE_TRELLIS_QUANT, 1);
    call_ctl!(ctx, AV1E_SET_ENABLE_DIST_WTD_COMP, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_DIFF_WTD_COMP, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_DUAL_FILTER, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_INTERINTRA_COMP, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_INTERINTRA_WEDGE, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_INTRA_EDGE_FILTER, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_INTRABC, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_MASKED_COMP, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_PAETH_INTRA, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_QM, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_RECT_PARTITIONS, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_RESTORATION, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_SMOOTH_INTERINTRA, 0);
    call_ctl!(ctx, AV1E_SET_ENABLE_TX64, 0);
    call_ctl!(ctx, AV1E_SET_MAX_REFERENCE_FRAMES, 3);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_uint;

    #[test]
    fn tile_log2_uses_c_uint_and_rounds_up() {
        let one_thread: c_uint = tile_log2(1);
        let three_threads: c_uint = tile_log2(3);
        let max_threads: c_uint = tile_log2(64);

        assert_eq!(one_thread, 0);
        assert_eq!(three_threads, 2);
        assert_eq!(max_threads, 6);
    }
}
