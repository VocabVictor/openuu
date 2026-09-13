use super::*;

impl TraitCapturer for IpcDrmCapturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        let deadline = Instant::now() + timeout;
        {
            let mut slot = self.shared.slot.lock().unwrap();
            loop {
                if slot.latest.is_some() || slot.ended.is_some() {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                let (guard, _timed_out) =
                    self.shared.cv.wait_timeout(slot, deadline - now).unwrap();
                slot = guard;
            }
            if let Some((w, h, fmt, buf)) = slot.latest.take() {
                drop(slot);
                // A layout change bumps the generation and is otherwise invisible here (mode
                // and framebuffer keep their size). Rebuild for the new transform; not counted
                // against health: the layout moved, the display did not fail.
                if scrap::wayland::display::wayland_snapshot_generation() != self.snapshot_gen {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("drm: display {} layout changed; rebuilding", self.display),
                    ));
                }
                // Frames arrive in scanout orientation, the session was sized rotated, so the
                // guard compares rotated dims. convert_to_yuv only refuses a LARGER source (a
                // smaller one leaves stale edges); first frame: CRTC mode vs scanout fb.
                let (fw, fh) = rotated_dims(self.transform, w, h);
                if self.session_size.is_some_and(|(sw, sh)| (fw, fh) != (sw, sh)) {
                    self.shared.slot.lock().unwrap().recycle(buf);
                    if !self.got_frame {
                        self.note_session_without_frame();
                    }
                    let (sw, sh) = self.session_size.unwrap_or_default();
                    let what = if self.got_frame {
                        "changed geometry mid-session"
                    } else {
                        "never matched its advertised geometry"
                    };
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} {what} ({sw}x{sh} -> {fw}x{fh}); rebuilding",
                            self.display
                        ),
                    ));
                }
                if self.transform == 0 {
                    let previous = std::mem::replace(&mut self.cur, buf);
                    self.shared.slot.lock().unwrap().recycle(previous);
                } else if !matches!(fmt, Pixfmt::BGRA | Pixfmt::RGBA) {
                    // Unreachable with today's producers (the convert path emits 4-byte pixels
                    // and the CPU path hardcodes BGRA); kept so a future non-4-byte producer
                    // fails the session instead of shearing the image.
                    self.shared.slot.lock().unwrap().recycle(buf);
                    if !self.got_frame {
                        self.note_session_without_frame();
                    }
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!(
                            "drm: display {} delivered {fmt:?} on a rotated output; rebuilding",
                            self.display
                        ),
                    ));
                } else {
                    unrotate_bgra(&buf, w, h, self.transform, &mut self.cur);
                    self.shared.slot.lock().unwrap().recycle(buf);
                }
                self.cur_w = fw;
                self.cur_h = fh;
                self.cur_fmt = fmt;
                if !self.got_frame {
                    // Clear ONLY the streak: `rapid_builds` is for a display that delivers a first
                    // frame then fails, and `prefer_cpu` is written on the recv thread.
                    self.got_frame = true;
                    if let Some(key) = &self.connector {
                        if let Some(h) = DRM_DISPLAY_HEALTH.lock().unwrap().get_mut(key) {
                            h.zero_frame_streak = 0;
                            h.demotes = 0;
                            h.since = Instant::now();
                            h.fallback_rejected = false;
                        }
                    }
                }
            } else {
                let err = slot
                    .ended
                    .clone()
                    .unwrap_or_else(|| "drm stream ended".to_owned());
                if !self.got_frame {
                    self.note_session_without_frame();
                }
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
        }
        Ok(Frame::PixelBuffer(PixelBuffer::new(
            &self.cur,
            self.cur_fmt,
            self.cur_w,
            self.cur_h,
        )))
    }
}
