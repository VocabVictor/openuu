use super::*;

impl EncoderApi for VpxEncoder {
    fn new(cfg: crate::codec::EncoderCfg, i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            crate::codec::EncoderCfg::VPX(config) => {
                let i = match config.codec {
                    VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_cx()),
                    VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_cx()),
                };
                let mut c = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
                call_vpx!(vpx_codec_enc_config_default(i, &mut c, 0));

                // https://www.webmproject.org/docs/encoder-parameters/
                // default: c.rc_min_quantizer = 0, c.rc_max_quantizer = 63
                // try rc_resize_allowed later

                c.g_w = config.width;
                c.g_h = config.height;
                c.g_timebase.num = 1;
                c.g_timebase.den = 1000; // Output timestamp precision
                c.rc_undershoot_pct = 95;
                // When the data buffer falls below this percentage of fullness, a dropped frame is indicated. Set the threshold to zero (0) to disable this feature.
                // In dynamic scenes, low bitrate gets low fps while high bitrate gets high fps.
                c.rc_dropframe_thresh = 25;
                c.g_threads = codec_thread_num(64) as _;
                c.g_error_resilient = VPX_ERROR_RESILIENT_DEFAULT;
                // https://developers.google.com/media/vp9/bitrate-modes/
                // Constant Bitrate mode (CBR) is recommended for live streaming with VP9.
                c.rc_end_usage = vpx_rc_mode::VPX_CBR;
                if let Some(keyframe_interval) = config.keyframe_interval {
                    c.kf_min_dist = 0;
                    c.kf_max_dist = keyframe_interval as _;
                } else {
                    c.kf_mode = vpx_kf_mode::VPX_KF_DISABLED; // reduce bandwidth a lot
                }

                let (q_min, q_max) = Self::calc_q_values(config.quality);
                c.rc_min_quantizer = q_min;
                c.rc_max_quantizer = q_max;
                c.rc_target_bitrate =
                    Self::bitrate(config.width as _, config.height as _, config.quality);
                // https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vp9/common/vp9_enums.h#29
                // https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vp8/vp8_cx_iface.c#282
                c.g_profile = if i444 && config.codec == VpxVideoCodecId::VP9 {
                    1
                } else {
                    0
                };

                /*
                The VPX encoder supports two-pass encoding for rate control purposes.
                In two-pass encoding, the entire encoding process is performed twice.
                The first pass generates new control parameters for the second pass.

                This approach enables the best PSNR at the same bit rate.
                */

                let mut ctx = Default::default();
                call_vpx!(vpx_codec_enc_init_ver(
                    &mut ctx,
                    i,
                    &c,
                    0,
                    VPX_ENCODER_ABI_VERSION as _
                ));

                if config.codec == VpxVideoCodecId::VP9 {
                    // set encoder internal speed settings
                    // in ffmpeg, it is --speed option
                    /*
                    set to 0 or a positive value 1-16, the codec will try to adapt its
                    complexity depending on the time it spends encoding. Increasing this
                    number will make the speed go up and the quality go down.
                    Negative values mean strict enforcement of this
                    while positive values are adaptive
                    */
                    /* https://developers.google.com/media/vp9/live-encoding
                    Speed 5 to 8 should be used for live / real-time encoding.
                    Lower numbers (5 or 6) are higher quality but require more CPU power.
                    Higher numbers (7 or 8) will be lower quality but more manageable for lower latency
                    use cases and also for lower CPU power devices such as mobile.
                    */
                    call_vpx!(vpx_codec_control_(&mut ctx, VP8E_SET_CPUUSED as _, 7,));
                    // set row level multi-threading
                    /*
                    as some people in comments and below have already commented,
                    more recent versions of libvpx support -row-mt 1 to enable tile row
                    multi-threading. This can increase the number of tiles by up to 4x in VP9
                    (since the max number of tile rows is 4, regardless of video height).
                    To enable this, use -tile-rows N where N is the number of tile rows in
                    log2 units (so -tile-rows 1 means 2 tile rows and -tile-rows 2 means 4 tile
                    rows). The total number of active threads will then be equal to
                    $tile_rows * $tile_columns
                    */
                    call_vpx!(vpx_codec_control_(
                        &mut ctx,
                        VP9E_SET_ROW_MT as _,
                        1 as c_int
                    ));

                    call_vpx!(vpx_codec_control_(
                        &mut ctx,
                        VP9E_SET_TILE_COLUMNS as _,
                        4 as c_int
                    ));
                } else if config.codec == VpxVideoCodecId::VP8 {
                    // https://github.com/webmproject/libvpx/blob/972149cafeb71d6f08df89e91a0130d6a38c4b15/vpx/vp8cx.h#L172
                    // https://groups.google.com/a/webmproject.org/g/webm-discuss/c/DJhSrmfQ61M
                    call_vpx!(vpx_codec_control_(&mut ctx, VP8E_SET_CPUUSED as _, 12,));
                }

                Ok(Self {
                    ctx,
                    width: config.width as _,
                    height: config.height as _,
                    id: config.codec,
                    i444,
                    yuvfmt: Self::get_yuvfmt(config.width, config.height, i444),
                })
            }
            _ => Err(anyhow!("encoder type mismatch")),
        }
    }

    fn encode_to_message(&mut self, input: EncodeInput, ms: i64) -> ResultType<VideoFrame> {
        let mut frames = Vec::new();
        for ref frame in self
            .encode(ms, input.yuv()?, STRIDE_ALIGN)
            .with_context(|| "Failed to encode")?
        {
            frames.push(VpxEncoder::create_frame(frame));
        }
        for ref frame in self.flush().with_context(|| "Failed to flush")? {
            frames.push(VpxEncoder::create_frame(frame));
        }

        // to-do: flush periodically, e.g. 1 second
        if frames.len() > 0 {
            Ok(VpxEncoder::create_video_frame(self.id, frames))
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    fn yuvfmt(&self) -> crate::EncodeYuvFormat {
        self.yuvfmt.clone()
    }

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool {
        false
    }

    fn set_quality(&mut self, ratio: f32) -> ResultType<()> {
        let mut c = unsafe { *self.ctx.config.enc.to_owned() };
        let (q_min, q_max) = Self::calc_q_values(ratio);
        c.rc_min_quantizer = q_min;
        c.rc_max_quantizer = q_max;
        c.rc_target_bitrate = Self::bitrate(self.width as _, self.height as _, ratio);
        call_vpx!(vpx_codec_enc_config_set(&mut self.ctx, &c));
        Ok(())
    }

    fn bitrate(&self) -> u32 {
        let c = unsafe { *self.ctx.config.enc.to_owned() };
        c.rc_target_bitrate
    }

    fn support_changing_quality(&self) -> bool {
        true
    }

    fn latency_free(&self) -> bool {
        true
    }

    fn is_hardware(&self) -> bool {
        false
    }

    fn disable(&self) {}
}
