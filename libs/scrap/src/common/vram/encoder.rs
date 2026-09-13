use super::*;

#[derive(Debug, Clone)]
pub struct VRamEncoderConfig {
    pub device: AdapterDevice,
    pub width: usize,
    pub height: usize,
    pub quality: f32,
    pub feature: FeatureContext,
    pub keyframe_interval: Option<usize>,
}

pub struct VRamEncoder {
    pub(super) encoder: Encoder,
    pub format: DataFormat,
    pub(super) ctx: EncodeContext,
    pub(super) bitrate: u32,
    pub(super) last_frame_len: usize,
    pub(super) same_bad_len_counter: usize,
}

impl EncoderApi for VRamEncoder {
    fn new(cfg: EncoderCfg, _i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            EncoderCfg::VRAM(config) => {
                let bitrate = Self::bitrate(
                    config.feature.data_format,
                    config.width,
                    config.height,
                    config.quality,
                );
                let gop = config.keyframe_interval.unwrap_or(MAX_GOP as _) as i32;
                let ctx = EncodeContext {
                    f: config.feature.clone(),
                    d: DynamicContext {
                        device: Some(config.device.device),
                        width: config.width as _,
                        height: config.height as _,
                        kbitrate: bitrate as _,
                        framerate: 30,
                        gop,
                    },
                };
                match Encoder::new(ctx.clone()) {
                    Ok(encoder) => Ok(VRamEncoder {
                        encoder,
                        ctx,
                        format: config.feature.data_format,
                        bitrate,
                        last_frame_len: 0,
                        same_bad_len_counter: 0,
                    }),
                    Err(_) => Err(anyhow!(format!("Failed to create encoder"))),
                }
            }
            _ => Err(anyhow!("encoder type mismatch")),
        }
    }

    fn encode_to_message(
        &mut self,
        frame: EncodeInput,
        ms: i64,
    ) -> ResultType<base::message_proto::VideoFrame> {
        let (texture, rotation) = frame.texture()?;
        if rotation != 0 {
            // to-do: support rotation
            // Both the encoder and display(w,h) information need to be changed.
            bail!("rotation not supported");
        }
        let mut vf = VideoFrame::new();
        let mut frames = Vec::new();
        for frame in self
            .encode(texture, ms)
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
            // This kind of problem is occurred after a period of time when using AMD encoding,
            // the encoding length is fixed at about 40, and the picture is still
            const MIN_BAD_LEN: usize = 100;
            const MAX_BAD_COUNTER: usize = 30;
            let this_frame_len = frames[0].data.len();
            if this_frame_len < MIN_BAD_LEN && this_frame_len == self.last_frame_len {
                self.same_bad_len_counter += 1;
                if self.same_bad_len_counter >= MAX_BAD_COUNTER {
                    log::info!(
                        "{} times encoding len is {}, switch",
                        self.same_bad_len_counter,
                        self.last_frame_len
                    );
                    bail!(crate::codec::ENCODE_NEED_SWITCH);
                }
            } else {
                self.same_bad_len_counter = 0;
            }
            self.last_frame_len = this_frame_len;
            let frames = EncodedVideoFrames {
                frames: frames.into(),
                ..Default::default()
            };
            match self.format {
                DataFormat::H264 => vf.set_h264s(frames),
                DataFormat::H265 => vf.set_h265s(frames),
                _ => bail!("{:?} not supported", self.format),
            }
            Ok(vf)
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    fn yuvfmt(&self) -> EncodeYuvFormat {
        // useless
        EncodeYuvFormat {
            pixfmt: Pixfmt::BGRA,
            w: self.ctx.d.width as _,
            h: self.ctx.d.height as _,
            stride: Vec::new(),
            u: 0,
            v: 0,
        }
    }

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool {
        true
    }

    fn set_quality(&mut self, ratio: f32) -> ResultType<()> {
        let bitrate = Self::bitrate(
            self.ctx.f.data_format,
            self.ctx.d.width as _,
            self.ctx.d.height as _,
            ratio,
        );
        if bitrate > 0 {
            if self.encoder.set_bitrate((bitrate) as _).is_ok() {
                self.bitrate = bitrate;
            }
        }
        Ok(())
    }

    fn set_fps(&mut self, fps: u32) -> ResultType<()> {
        let fps = fps as i32;
        if fps <= 0 || fps == self.ctx.d.framerate {
            return Ok(());
        }
        self.encoder
            .set_framerate(fps)
            .map_err(|err| anyhow!("set_framerate({fps}) failed: {err}"))?;
        self.ctx.d.framerate = fps;
        Ok(())
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn support_changing_quality(&self) -> bool {
        true
    }

    fn latency_free(&self) -> bool {
        true
    }

    fn is_hardware(&self) -> bool {
        true
    }

    fn disable(&self) {
        HwCodecConfig::clear(true, true);
    }
}

impl VRamEncoder {
    pub fn try_get(device: &AdapterDevice, format: CodecFormat) -> Option<FeatureContext> {
        let v: Vec<_> = Self::available(format)
            .drain(..)
            .filter(|e| e.luid == device.luid)
            .collect();
        if v.len() > 0 {
            // prefer ffmpeg
            if let Some(ctx) = v.iter().find(|c| c.driver == Driver::FFMPEG) {
                return Some(ctx.clone());
            }
            Some(v[0].clone())
        } else {
            None
        }
    }

    pub fn available(format: CodecFormat) -> Vec<FeatureContext> {
        let fallbacks = FALLBACK_GDI_DISPLAYS.lock().unwrap().clone();
        if !fallbacks.is_empty() {
            log::info!("fallback gdi displays not empty: {fallbacks:?}");
            return vec![];
        }
        let not_use = ENOCDE_NOT_USE.lock().unwrap().clone();
        if not_use.values().any(|not_use| *not_use) {
            log::info!("currently not use vram encoders: {not_use:?}");
            return vec![];
        }
        let data_format = match format {
            CodecFormat::H264 => DataFormat::H264,
            CodecFormat::H265 => DataFormat::H265,
            _ => return vec![],
        };
        let v: Vec<_> = crate::hwcodec::HwCodecConfig::get()
            .vram_encode
            .drain(..)
            .filter(|c| c.data_format == data_format)
            .collect();
        if crate::hwcodec::HwRamEncoder::try_get(format).is_some() {
            // has fallback, no need to require all adapters support
            v
        } else {
            let Ok(displays) = crate::Display::all() else {
                log::error!("failed to get displays");
                return vec![];
            };
            if displays.is_empty() {
                log::error!("no display found");
                return vec![];
            }
            let luids = displays
                .iter()
                .map(|d| d.adapter_luid())
                .collect::<Vec<_>>();
            if luids
                .iter()
                .all(|luid| v.iter().any(|f| Some(f.luid) == *luid))
            {
                v
            } else {
                log::info!("not all adapters support {data_format:?}, luids = {luids:?}");
                vec![]
            }
        }
    }

    pub fn encode(&mut self, texture: *mut c_void, ms: i64) -> ResultType<Vec<EncodeFrame>> {
        match self.encoder.encode(texture, ms) {
            Ok(v) => {
                let mut data = Vec::<EncodeFrame>::new();
                data.append(v);
                Ok(data)
            }
            Err(_) => Ok(Vec::<EncodeFrame>::new()),
        }
    }

    pub fn bitrate(fmt: DataFormat, width: usize, height: usize, ratio: f32) -> u32 {
        crate::hwcodec::HwRamEncoder::calc_bitrate(width, height, ratio, fmt == DataFormat::H264)
    }

    pub fn set_not_use(video_service_name: String, not_use: bool) {
        log::info!("set {video_service_name} not use vram encode to {not_use}");
        ENOCDE_NOT_USE
            .lock()
            .unwrap()
            .insert(video_service_name, not_use);
    }

    pub fn set_fallback_gdi(video_service_name: String, fallback: bool) {
        if fallback {
            FALLBACK_GDI_DISPLAYS
                .lock()
                .unwrap()
                .insert(video_service_name);
        } else {
            FALLBACK_GDI_DISPLAYS
                .lock()
                .unwrap()
                .remove(&video_service_name);
        }
    }
}
