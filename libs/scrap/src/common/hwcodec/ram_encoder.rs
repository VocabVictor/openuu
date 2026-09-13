use super::*;

#[derive(Debug, Clone)]
pub struct HwRamEncoderConfig {
    pub name: String,
    pub mc_name: Option<String>,
    pub width: usize,
    pub height: usize,
    pub quality: f32,
    pub keyframe_interval: Option<usize>,
}

pub struct HwRamEncoder {
    pub(super) encoder: Encoder,
    pub format: DataFormat,
    pub pixfmt: AVPixelFormat,
    pub(super) bitrate: u32, //kbs
    pub(super) config: HwRamEncoderConfig,
    /// What QoS paces the capture at; the encoder itself keeps assuming DEFAULT_FPS.
    pub(super) fps: u32,
}

impl EncoderApi for HwRamEncoder {
    fn new(cfg: EncoderCfg, _i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            EncoderCfg::HWRAM(config) => {
                let rc = Self::rate_control(&config);
                let mut bitrate =
                    Self::bitrate(&config.name, config.width, config.height, config.quality);
                bitrate = Self::check_bitrate_range(&config, bitrate);
                let gop = config.keyframe_interval.unwrap_or(DEFAULT_GOP as _) as i32;
                let ctx = EncodeContext {
                    name: config.name.clone(),
                    mc_name: config.mc_name.clone(),
                    width: config.width as _,
                    height: config.height as _,
                    pixfmt: DEFAULT_PIXFMT,
                    align: HW_STRIDE_ALIGN as _,
                    kbs: bitrate as i32,
                    fps: DEFAULT_FPS,
                    gop,
                    quality: DEFAULT_HW_QUALITY,
                    rc,
                    q: -1,
                    thread_count: codec_thread_num(16) as _, // ffmpeg's thread_count is used for cpu
                };
                let format = match Encoder::format_from_name(config.name.clone()) {
                    Ok(format) => format,
                    Err(_) => {
                        return Err(anyhow!(format!(
                            "failed to get format from name:{}",
                            config.name
                        )))
                    }
                };
                match Encoder::new(ctx.clone()) {
                    Ok(encoder) => Ok(HwRamEncoder {
                        encoder,
                        format,
                        pixfmt: ctx.pixfmt,
                        bitrate,
                        config,
                        fps: DEFAULT_FPS as u32,
                    }),
                    Err(_) => Err(anyhow!(format!("Failed to create encoder"))),
                }
            }
            _ => Err(anyhow!("encoder type mismatch")),
        }
    }

    fn encode_to_message(&mut self, input: EncodeInput, ms: i64) -> ResultType<VideoFrame> {
        let mut vf = VideoFrame::new();
        let mut frames = Vec::new();
        for frame in self
            .encode(input.yuv()?, ms)
            .with_context(|| "Failed to encode")?
        {
            frames.push(EncodedVideoFrame {
                data: Bytes::from(frame.data),
                pts: frame.pts,
                key: frame.key == 1,
                ..Default::default()
            });
        }
        if frames.len() > 0 {
            let frames = EncodedVideoFrames {
                frames: frames.into(),
                ..Default::default()
            };
            match self.format {
                DataFormat::H264 => vf.set_h264s(frames),
                DataFormat::H265 => vf.set_h265s(frames),
                _ => bail!("unsupported format: {:?}", self.format),
            }
            Ok(vf)
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    fn yuvfmt(&self) -> crate::EncodeYuvFormat {
        let pixfmt = if self.pixfmt == AVPixelFormat::AV_PIX_FMT_NV12 {
            Pixfmt::NV12
        } else {
            Pixfmt::I420
        };
        let stride = self
            .encoder
            .linesize
            .clone()
            .drain(..)
            .map(|i| i as usize)
            .collect();
        crate::EncodeYuvFormat {
            pixfmt,
            w: self.encoder.ctx.width as _,
            h: self.encoder.ctx.height as _,
            stride,
            u: self.encoder.offset[0] as _,
            v: if pixfmt == Pixfmt::NV12 {
                0
            } else {
                self.encoder.offset[1] as _
            },
        }
    }

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool {
        false
    }

    fn set_quality(&mut self, ratio: f32) -> ResultType<()> {
        let mut bitrate = Self::bitrate(
            &self.config.name,
            self.config.width,
            self.config.height,
            ratio,
        );
        if bitrate > 0 {
            bitrate = Self::check_bitrate_range(&self.config, bitrate);
            self.bitrate = bitrate;
            self.apply_bitrate();
        }
        self.config.quality = ratio;
        Ok(())
    }

    /// ffmpeg's rate control has no frame-rate setter, so the target is scaled for the
    /// fps the capture loop really runs at; `bitrate()` still reports the target QoS asked for.
    fn set_fps(&mut self, fps: u32) -> ResultType<()> {
        if fps == 0 || fps == self.fps {
            return Ok(());
        }
        self.fps = fps;
        self.apply_bitrate();
        Ok(())
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn support_changing_quality(&self) -> bool {
        ["vaapi"].iter().all(|&x| !self.config.name.contains(x))
    }

    fn latency_free(&self) -> bool {
        ["mediacodec", "videotoolbox"]
            .iter()
            .all(|&x| !self.config.name.contains(x))
    }

    fn is_hardware(&self) -> bool {
        true
    }

    fn disable(&self) {
        HwCodecConfig::clear(false, true);
    }
}

impl HwRamEncoder {
    pub fn try_get(format: CodecFormat) -> Option<CodecInfo> {
        let mut info = None;
        let best = CodecInfo::prioritized(HwCodecConfig::get().ram_encode);
        match format {
            CodecFormat::H264 => {
                if let Some(v) = best.h264 {
                    info = Some(v);
                }
            }
            CodecFormat::H265 => {
                if let Some(v) = best.h265 {
                    info = Some(v);
                }
            }
            _ => {}
        }
        info
    }

    pub fn encode(&mut self, yuv: &[u8], ms: i64) -> ResultType<Vec<EncodeFrame>> {
        match self.encoder.encode(yuv, ms) {
            Ok(v) => {
                let mut data = Vec::<EncodeFrame>::new();
                data.append(v);
                Ok(data)
            }
            Err(_) => Ok(Vec::<EncodeFrame>::new()),
        }
    }

    pub(super) fn rate_control(_config: &HwRamEncoderConfig) -> RateControl {
        #[cfg(target_os = "android")]
        if _config.name.contains("mediacodec") {
            return RC_VBR;
        }
        RC_CBR
    }

    pub fn bitrate(name: &str, width: usize, height: usize, ratio: f32) -> u32 {
        Self::calc_bitrate(width, height, ratio, name.contains("h264"))
    }

    pub fn calc_bitrate(width: usize, height: usize, ratio: f32, h264: bool) -> u32 {
        let base = base_bitrate(width as _, height as _) as f32 * ratio;
        let threshold = 2000.0;
        let decay_rate = 0.001; // 1000 * 0.001 = 1
        let factor: f32 = if cfg!(target_os = "android") {
            // https://stackoverflow.com/questions/26110337/what-are-valid-bit-rates-to-set-for-mediacodec?rq=3
            if base > threshold {
                1.0 + 4.0 / (1.0 + (base - threshold) * decay_rate)
            } else {
                5.0
            }
        } else if h264 {
            if base > threshold {
                1.0 + 1.0 / (1.0 + (base - threshold) * decay_rate)
            } else {
                2.0
            }
        } else {
            if base > threshold {
                1.0 + 0.5 / (1.0 + (base - threshold) * decay_rate)
            } else {
                1.5
            }
        };
        (base * factor) as u32
    }

    pub fn check_bitrate_range(_config: &HwRamEncoderConfig, bitrate: u32) -> u32 {
        #[cfg(target_os = "android")]
        if _config.name.contains("mediacodec") {
            let info = crate::android::ffi::get_codec_info();
            if let Some(info) = info {
                if let Some(codec) = info
                    .codecs
                    .iter()
                    .find(|c| Some(c.name.clone()) == _config.mc_name && c.is_encoder)
                {
                    if codec.max_bitrate > codec.min_bitrate {
                        if bitrate > codec.max_bitrate {
                            return codec.max_bitrate;
                        }
                        if bitrate < codec.min_bitrate {
                            return codec.min_bitrate;
                        }
                    }
                }
            }
        }
        bitrate
    }
}

impl HwRamEncoder {
    fn apply_bitrate(&mut self) {
        let kbs = fps_compensated_kbs(self.bitrate, DEFAULT_FPS as u32, self.fps);
        let kbs = Self::check_bitrate_range(&self.config, kbs);
        self.encoder.set_bitrate(kbs as _).ok();
    }
}
