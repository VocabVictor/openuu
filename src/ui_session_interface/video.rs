use super::*;

impl<T: InvokeUiSession> Session<T> {
    #[cfg(feature = "flutter")]
    pub fn refresh_video(&self, display: i32) {
        if crate::common::is_support_multi_ui_session_num(self.lc.read().unwrap().version) {
            self.send(Data::Message(LoginConfigHandler::refresh_display(
                display as _,
            )));
        } else {
            self.send(Data::Message(LoginConfigHandler::refresh()));
        }
    }

    #[cfg(not(feature = "flutter"))]
    pub fn refresh_video(&self, _display: i32) {
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    pub fn toggle_virtual_display(&self, index: i32, on: bool) {
        let mut misc = Misc::new();
        misc.set_toggle_virtual_display(ToggleVirtualDisplay {
            display: index,
            on,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn record_screen(&self, start: bool) {
        self.send(Data::RecordScreen(start));
    }

    pub fn is_screenshot_supported(&self) -> bool {
        crate::common::is_support_screenshot_num(self.lc.read().unwrap().version)
    }

    pub fn take_screenshot(&self, display: i32, sid: String) {
        self.send(Data::TakeScreenshot((display, sid)));
    }

    pub fn is_recording(&self) -> bool {
        self.lc.read().unwrap().record_state
    }

    pub fn save_custom_image_quality(&self, custom_image_quality: i32) {
        let msg = self
            .lc
            .write()
            .unwrap()
            .save_custom_image_quality(custom_image_quality);
        self.send(Data::Message(msg));
    }

    pub fn save_image_quality(&self, value: String) {
        let msg = self.lc.write().unwrap().save_image_quality(value.clone());
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
        if value != "custom" {
            let last_auto_fps = self.lc.read().unwrap().last_auto_fps;
            if last_auto_fps.unwrap_or(usize::MAX) >= 30 {
                // non custom quality use 30 fps
                let msg = self.lc.write().unwrap().set_custom_fps(30, false);
                self.send(Data::Message(msg));
            }
        }
    }

    pub fn save_trackpad_speed(&self, trackpad_speed: i32) {
        self.lc.write().unwrap().save_trackpad_speed(trackpad_speed);
    }

    pub fn set_custom_fps(&self, custom_fps: i32) {
        let msg = self.lc.write().unwrap().set_custom_fps(custom_fps, true);
        self.send(Data::Message(msg));
    }

    pub fn get_remember(&self) -> bool {
        self.lc.read().unwrap().remember
    }

    #[cfg(not(feature = "flutter"))]
    pub fn set_write_override(
        &mut self,
        job_id: i32,
        file_num: i32,
        is_override: bool,
        remember: bool,
        is_upload: bool,
    ) -> bool {
        self.send(Data::SetConfirmOverrideFile((
            job_id,
            file_num,
            is_override,
            remember,
            is_upload,
        )));
        true
    }

    pub fn alternative_codecs(&self) -> (bool, bool, bool, bool) {
        let luid = self.lc.read().unwrap().adapter_luid;
        let mark_unsupported = self.lc.read().unwrap().mark_unsupported.clone();
        let decoder = scrap::codec::Decoder::supported_decodings(
            None,
            use_texture_render(),
            luid,
            &mark_unsupported,
        );
        let mut vp8 = decoder.ability_vp8 > 0;
        let mut av1 = decoder.ability_av1 > 0;
        let mut h264 = decoder.ability_h264 > 0;
        let mut h265 = decoder.ability_h265 > 0;
        let enc = &self.lc.read().unwrap().supported_encoding;
        vp8 = vp8 && enc.vp8;
        av1 = av1 && enc.av1;
        h264 = h264 && enc.h264;
        h265 = h265 && enc.h265;
        (vp8, av1, h264, h265)
    }

    pub fn update_supported_decodings(&self) {
        let msg = self.lc.write().unwrap().update_supported_decodings();
        self.send(Data::Message(msg));
    }

    pub fn use_texture_render_changed(&self) {
        self.send(Data::ResetDecoder(None));
        self.update_supported_decodings();
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    pub fn capture_displays(&self, add: Vec<i32>, sub: Vec<i32>, set: Vec<i32>) {
        let mut misc = Misc::new();
        misc.set_capture_displays(CaptureDisplays {
            add,
            sub,
            set,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn switch_display(&self, display: i32) {
        let (w, h) = match self.lc.read().unwrap().get_custom_resolution(display) {
            Some((w, h)) => (w, h),
            None => (0, 0),
        };

        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display,
            width: w,
            height: h,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));

        if !use_texture_render() {
            self.capture_displays(vec![], vec![], vec![display]);
        }
    }

    fn set_custom_resolution(&self, display: &SwitchDisplay) {
        if display.width == display.original_resolution.width
            && display.height == display.original_resolution.height
        {
            self.lc
                .write()
                .unwrap()
                .set_custom_resolution(display.display, None);
        } else {
            let last_change_display = self.last_change_display.lock().unwrap();
            if last_change_display.display == display.display {
                let wh = if last_change_display.is_the_same_record(
                    display.display,
                    display.width,
                    display.height,
                ) {
                    Some((display.width, display.height))
                } else {
                    // display origin is changed, or some other events.
                    None
                };
                self.lc
                    .write()
                    .unwrap()
                    .set_custom_resolution(display.display, wh);
            }
        }
    }

    #[inline]
    pub fn handle_peer_switch_display(&self, display: &SwitchDisplay) {
        self.ui_handler.switch_display(display);
        self.set_custom_resolution(display);
    }

    #[inline]
    pub fn change_resolution(&self, display: i32, width: i32, height: i32) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        *self.last_change_display.lock().unwrap() =
            ChangeDisplayRecord::new(display, width, height);
        self.do_change_resolution(display, width, height);
    }

    #[inline]
    pub(super) fn try_change_init_resolution(&self, display: i32) {
        let Some((w, h)) = self.lc.read().unwrap().get_custom_resolution(display) else {
            return;
        };
        self.change_resolution(display, w, h);
    }

    fn do_change_resolution(&self, display: i32, width: i32, height: i32) {
        let mut misc = Misc::new();
        let resolution = Resolution {
            width,
            height,
            ..Default::default()
        };
        if crate::common::is_support_multi_ui_session_num(self.lc.read().unwrap().version) {
            misc.set_change_display_resolution(DisplayResolution {
                display,
                resolution: Some(resolution).into(),
                ..Default::default()
            });
        } else {
            misc.set_change_resolution(resolution);
        }
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }
}
