use super::*;

/// Video handler for the [`Client`].
pub struct VideoHandler {
    pub(super) decoder: Decoder,
    pub rgb: ImageRgb,
    pub texture: ImageTexture,
    pub(super) recorder: Arc<Mutex<Option<Recorder>>>,
    pub(super) record: bool,
    pub(super) _display: usize, // useful for debug
    pub(super) fail_counter: usize,
    pub(super) first_frame: bool,
}

impl VideoHandler {
    #[cfg(feature = "flutter")]
    pub fn get_adapter_luid() -> Option<i64> {
        crate::flutter::get_adapter_luid()
    }

    /// Create a new video handler.
    pub fn new(format: CodecFormat, _display: usize) -> Self {
        let luid = Self::get_adapter_luid();
        log::info!("new video handler for display #{_display}, format: {format:?}, luid: {luid:?}");
        let rgba_format =
            if cfg!(feature = "flutter") && (cfg!(windows) || cfg!(target_os = "linux")) {
                ImageFormat::ABGR
            } else {
                ImageFormat::ARGB
            };
        VideoHandler {
            decoder: Decoder::new(format, luid),
            rgb: ImageRgb::new(rgba_format, crate::get_dst_align_rgba()),
            texture: Default::default(),
            recorder: Default::default(),
            record: false,
            _display,
            fail_counter: 0,
            first_frame: true,
        }
    }

    /// Handle a new video frame.
    #[inline]
    pub fn handle_frame(
        &mut self,
        vf: VideoFrame,
        pixelbuffer: &mut bool,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        let format = CodecFormat::from(&vf);
        if format != self.decoder.format() {
            self.reset(Some(format));
        }
        match &vf.union {
            Some(frame) => {
                let res = self.decoder.handle_video_frame(
                    frame,
                    &mut self.rgb,
                    &mut self.texture,
                    pixelbuffer,
                    chroma,
                );
                if res.as_ref().is_ok_and(|x| *x) {
                    self.fail_counter = 0;
                } else {
                    if self.fail_counter < usize::MAX {
                        if self.first_frame && self.fail_counter < MAX_DECODE_FAIL_COUNTER {
                            log::error!("decode first frame failed");
                            self.fail_counter = MAX_DECODE_FAIL_COUNTER;
                        } else {
                            self.fail_counter += 1;
                        }
                        log::error!(
                            "Failed to handle video frame, fail counter: {}",
                            self.fail_counter
                        );
                    }
                }
                self.first_frame = false;
                if self.record {
                    self.recorder.lock().unwrap().as_mut().map(|r| {
                        let (w, h) = if *pixelbuffer {
                            (self.rgb.w, self.rgb.h)
                        } else {
                            (self.texture.w, self.texture.h)
                        };
                        r.write_frame(frame, w, h).ok();
                    });
                }
                res
            }
            _ => Ok(false),
        }
    }

    /// Reset the decoder, change format if it is Some
    pub fn reset(&mut self, format: Option<CodecFormat>) {
        log::info!(
            "reset video handler for display #{}, format: {format:?}",
            self._display
        );
        #[cfg(target_os = "macos")]
        self.rgb.set_align(crate::get_dst_align_rgba());
        let luid = Self::get_adapter_luid();
        let format = format.unwrap_or(self.decoder.format());
        self.decoder = Decoder::new(format, luid);
        self.fail_counter = 0;
        self.first_frame = true;
    }

    /// Start or stop screen record.
    pub fn record_screen(&mut self, start: bool, id: String, display_idx: usize, camera: bool) {
        self.record = false;
        if start {
            self.recorder = Recorder::new(RecorderContext {
                server: false,
                id,
                dir: crate::ui_interface::video_save_directory(false),
                display_idx,
                camera,
                tx: None,
            })
            .map_or(Default::default(), |r| Arc::new(Mutex::new(Some(r))));
        } else {
            self.recorder = Default::default();
        }

        self.record = start;
    }
}
