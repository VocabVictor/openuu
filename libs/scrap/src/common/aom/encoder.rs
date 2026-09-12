use super::*;

impl EncoderApi for AomEncoder {
    fn new(cfg: crate::codec::EncoderCfg, i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            crate::codec::EncoderCfg::AOM(config) => {
                let i = call_aom_ptr!(aom_codec_av1_cx());
                let c = webrtc::enc_cfg(i, config, i444)?;

                let mut ctx = Default::default();
                // Flag options: AOM_CODEC_USE_PSNR and AOM_CODEC_USE_HIGHBITDEPTH
                let flags: aom_codec_flags_t = 0;
                call_aom!(aom_codec_enc_init_ver(
                    &mut ctx,
                    i,
                    &c,
                    flags,
                    AOM_ENCODER_ABI_VERSION as _
                ));
                webrtc::set_controls(&mut ctx, &c)?;
                Ok(Self {
                    ctx,
                    width: config.width as _,
                    height: config.height as _,
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
            frames.push(Self::create_frame(frame));
        }
        if frames.len() > 0 {
            Ok(Self::create_video_frame(frames))
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
        call_aom!(aom_codec_enc_config_set(&mut self.ctx, &c));
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

impl AomEncoder {
    pub fn encode<'a>(&'a mut self, ms: i64, data: &[u8], stride_align: usize) -> Result<EncodeFrames<'a>> {
        let bpp = if self.i444 { 24 } else { 12 };
        if data.len() < self.width * self.height * bpp / 8 {
            return Err(Error::FailedCall("len not enough".to_string()));
        }
        let fmt = if self.i444 {
            aom_img_fmt::AOM_IMG_FMT_I444
        } else {
            aom_img_fmt::AOM_IMG_FMT_I420
        };

        let mut image = Default::default();
        call_aom_ptr!(aom_img_wrap(
            &mut image,
            fmt,
            self.width as _,
            self.height as _,
            stride_align as _,
            data.as_ptr() as _,
        ));
        let pts = webrtc::kTimeBaseDen / 1000 * ms;
        let duration = webrtc::kTimeBaseDen / 1000;
        call_aom!(aom_codec_encode(
            &mut self.ctx,
            &image,
            pts as _,
            duration as _, // Duration
            0,             // Flags
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    #[inline]
    pub fn create_video_frame(frames: Vec<EncodedVideoFrame>) -> VideoFrame {
        let mut vf = VideoFrame::new();
        let av1s = EncodedVideoFrames {
            frames: frames.into(),
            ..Default::default()
        };
        vf.set_av1s(av1s);
        vf
    }

    #[inline]
    pub(super) fn create_frame(frame: &EncodeFrame) -> EncodedVideoFrame {
        EncodedVideoFrame {
            data: Bytes::from(frame.data.to_vec()),
            key: frame.key,
            pts: frame.pts,
            ..Default::default()
        }
    }

    pub(super) fn bitrate(width: u32, height: u32, ratio: f32) -> u32 {
        let bitrate = base_bitrate(width, height) as f32;
        (bitrate * ratio) as u32
    }

    #[inline]
    pub(super) fn calc_q_values(ratio: f32) -> (u32, u32) {
        let b = (ratio * 100.0) as u32;
        let b = std::cmp::min(b, 200);
        let q_min1 = 24;
        let q_min2 = 5;
        let q_max1 = 45;
        let q_max2 = 25;

        let t = b as f32 / 200.0;

        let mut q_min: u32 = ((1.0 - t) * q_min1 as f32 + t * q_min2 as f32).round() as u32;
        let mut q_max = ((1.0 - t) * q_max1 as f32 + t * q_max2 as f32).round() as u32;

        q_min = q_min.clamp(q_min2, q_min1);
        q_max = q_max.clamp(q_max2, q_max1);

        (q_min, q_max)
    }

    pub(super) fn get_yuvfmt(width: u32, height: u32, i444: bool) -> EncodeYuvFormat {
        let mut img = Default::default();
        let fmt = if i444 {
            aom_img_fmt::AOM_IMG_FMT_I444
        } else {
            aom_img_fmt::AOM_IMG_FMT_I420
        };
        unsafe {
            aom_img_wrap(
                &mut img,
                fmt,
                width as _,
                height as _,
                crate::STRIDE_ALIGN as _,
                0x1 as _,
            );
        }
        let pixfmt = if i444 { Pixfmt::I444 } else { Pixfmt::I420 };
        EncodeYuvFormat {
            pixfmt,
            w: img.w as _,
            h: img.h as _,
            stride: img.stride.map(|s| s as usize).to_vec(),
            u: img.planes[1] as usize - img.planes[0] as usize,
            v: img.planes[2] as usize - img.planes[0] as usize,
        }
    }
}

impl Drop for AomEncoder {
    fn drop(&mut self) {
        unsafe {
            let result = aom_codec_destroy(&mut self.ctx);
            if result != aom_codec_err_t::AOM_CODEC_OK {
                panic!("failed to destroy aom codec");
            }
        }
    }
}
