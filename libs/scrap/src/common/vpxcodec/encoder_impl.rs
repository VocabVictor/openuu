use super::*;

impl VpxEncoder {
    pub fn encode<'a>(&'a mut self, pts: i64, data: &[u8], stride_align: usize) -> Result<EncodeFrames<'a>> {
        let bpp = if self.i444 { 24 } else { 12 };
        if data.len() < self.width * self.height * bpp / 8 {
            return Err(Error::FailedCall("len not enough".to_string()));
        }
        let fmt = if self.i444 {
            vpx_img_fmt::VPX_IMG_FMT_I444
        } else {
            vpx_img_fmt::VPX_IMG_FMT_I420
        };

        let mut image = Default::default();
        call_vpx_ptr!(vpx_img_wrap(
            &mut image,
            fmt,
            self.width as _,
            self.height as _,
            stride_align as _,
            data.as_ptr() as _,
        ));

        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            &image,
            pts as _,
            1, // Duration
            0, // Flags
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the encoder to return any pending packets
    pub fn flush<'a>(&'a mut self) -> Result<EncodeFrames<'a>> {
        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            ptr::null(),
            -1, // PTS
            1,  // Duration
            0,  // Flags
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    #[inline]
    pub fn create_video_frame(
        codec_id: VpxVideoCodecId,
        frames: Vec<EncodedVideoFrame>,
    ) -> VideoFrame {
        let mut vf = VideoFrame::new();
        let vpxs = EncodedVideoFrames {
            frames: frames.into(),
            ..Default::default()
        };
        match codec_id {
            VpxVideoCodecId::VP8 => vf.set_vp8s(vpxs),
            VpxVideoCodecId::VP9 => vf.set_vp9s(vpxs),
        }
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
        let q_min1 = 36;
        let q_min2 = 0;
        let q_max1 = 56;
        let q_max2 = 37;

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
            vpx_img_fmt::VPX_IMG_FMT_I444
        } else {
            vpx_img_fmt::VPX_IMG_FMT_I420
        };
        unsafe {
            vpx_img_wrap(
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

impl Drop for VpxEncoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}
