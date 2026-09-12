use super::*;

impl Recorder {
    pub fn new(ctx: RecorderContext) -> ResultType<Self> {
        Ok(Self {
            inner: None,
            ctx,
            ctx2: None,
            pts: None,
            check_failed: false,
        })
    }

    pub(super) fn check(&mut self, w: usize, h: usize, format: CodecFormat) -> ResultType<()> {
        match self.ctx2 {
            Some(ref ctx2) => {
                if ctx2.width != w || ctx2.height != h || ctx2.format != format {
                    let mut ctx2 = RecorderContext2 {
                        width: w,
                        height: h,
                        format,
                        filename: Default::default(),
                    };
                    ctx2.set_filename(&self.ctx)?;
                    self.ctx2 = Some(ctx2);
                    self.inner = None;
                }
            }
            None => {
                let mut ctx2 = RecorderContext2 {
                    width: w,
                    height: h,
                    format,
                    filename: Default::default(),
                };
                ctx2.set_filename(&self.ctx)?;
                self.ctx2 = Some(ctx2);
                self.inner = None;
            }
        }
        let Some(ctx2) = &self.ctx2 else {
            bail!("ctx2 is None");
        };
        if self.inner.is_none() {
            self.inner = match format {
                CodecFormat::VP8 | CodecFormat::VP9 | CodecFormat::AV1 => Some(Box::new(
                    WebmRecorder::new(self.ctx.clone(), (*ctx2).clone())?,
                )),
                #[cfg(feature = "hwcodec")]
                _ => Some(Box::new(HwRecorder::new(
                    self.ctx.clone(),
                    (*ctx2).clone(),
                )?)),
                #[cfg(not(feature = "hwcodec"))]
                _ => bail!("unsupported codec type"),
            };
            // pts is None when new inner is created
            self.pts = None;
            self.send_state(RecordState::NewFile(ctx2.filename.clone()));
        }
        Ok(())
    }

    pub fn write_message(&mut self, msg: &Message, w: usize, h: usize) {
        if let Some(message::Union::VideoFrame(vf)) = &msg.union {
            if let Some(frame) = &vf.union {
                self.write_frame(frame, w, h).ok();
            }
        }
    }

    pub fn write_frame(
        &mut self,
        frame: &video_frame::Union,
        w: usize,
        h: usize,
    ) -> ResultType<()> {
        if self.check_failed {
            bail!("check failed");
        }
        let format = CodecFormat::from(frame);
        if format == CodecFormat::Unknown {
            bail!("unsupported frame type");
        }
        let res = self.check(w, h, format);
        if res.is_err() {
            self.check_failed = true;
            log::error!("check failed: {:?}", res);
            res?;
        }
        match frame {
            video_frame::Union::Vp8s(vp8s) => {
                for f in vp8s.frames.iter() {
                    self.check_pts(f.pts, f.key, w, h, format)?;
                    self.as_mut().map(|x| x.write_video(f));
                }
            }
            video_frame::Union::Vp9s(vp9s) => {
                for f in vp9s.frames.iter() {
                    self.check_pts(f.pts, f.key, w, h, format)?;
                    self.as_mut().map(|x| x.write_video(f));
                }
            }
            video_frame::Union::Av1s(av1s) => {
                for f in av1s.frames.iter() {
                    self.check_pts(f.pts, f.key, w, h, format)?;
                    self.as_mut().map(|x| x.write_video(f));
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H264s(h264s) => {
                for f in h264s.frames.iter() {
                    self.check_pts(f.pts, f.key, w, h, format)?;
                    self.as_mut().map(|x| x.write_video(f));
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H265s(h265s) => {
                for f in h265s.frames.iter() {
                    self.check_pts(f.pts, f.key, w, h, format)?;
                    self.as_mut().map(|x| x.write_video(f));
                }
            }
            _ => bail!("unsupported frame type"),
        }
        self.send_state(RecordState::NewFrame);
        Ok(())
    }

    pub(super) fn check_pts(
        &mut self,
        pts: i64,
        key: bool,
        w: usize,
        h: usize,
        format: CodecFormat,
    ) -> ResultType<()> {
        // https://stackoverflow.com/questions/76379101/how-to-create-one-playable-webm-file-from-two-different-video-tracks-with-same-c
        if self.pts.is_none() && !key {
            bail!("first frame is not key frame");
        }
        let old_pts = self.pts;
        self.pts = Some(pts);
        if old_pts.clone().unwrap_or_default() > pts {
            log::info!("pts {:?} -> {}, change record filename", old_pts, pts);
            self.inner = None;
            self.ctx2 = None;
            let res = self.check(w, h, format);
            if res.is_err() {
                self.check_failed = true;
                log::error!("check failed: {:?}", res);
                res?;
            }
            self.pts = Some(pts);
        }
        Ok(())
    }

    pub(super) fn send_state(&self, state: RecordState) {
        self.ctx.tx.as_ref().map(|tx| tx.send(state));
    }
}
