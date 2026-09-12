use super::*;

impl SessionHandler {
    pub fn on_waiting_for_image_dialog_show(&self) {
        self.renderer.reset_all_display_render_type();
        // rgba array render will notify every frame
    }
}

impl FlutterHandler {
    /// Push an event to all the event queues.
    /// An event is stored as json in the event queues.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the event.
    /// * `event` - Fields of the event content.
    pub fn push_event<V>(&self, name: &str, event: &[(&str, V)], excludes: &[&SessionID])
    where
        V: Sized + Serialize + Clone,
    {
        self.push_event_(name, event, &[], excludes);
    }

    pub fn push_event_to<V>(&self, name: &str, event: &[(&str, V)], include: &[&SessionID])
    where
        V: Sized + Serialize + Clone,
    {
        self.push_event_(name, event, include, &[]);
    }

    pub fn push_event_<V>(
        &self,
        name: &str,
        event: &[(&str, V)],
        includes: &[&SessionID],
        excludes: &[&SessionID],
    ) where
        V: Sized + Serialize + Clone,
    {
        let mut h: HashMap<&str, serde_json::Value> =
            event.iter().map(|(k, v)| (*k, json!(*v))).collect();
        debug_assert!(h.get("name").is_none());
        h.insert("name", json!(name));
        let out = serde_json::ser::to_string(&h).unwrap_or("".to_owned());
        for (sid, session) in self.session_handlers.read().unwrap().iter() {
            let mut push = false;
            if includes.is_empty() {
                if !excludes.contains(&sid) {
                    push = true;
                }
            } else {
                if includes.contains(&sid) {
                    push = true;
                }
            }
            if push {
                if let Some(stream) = &session.event_stream {
                    stream.add(EventToUI::Event(out.clone()));
                }
            }
        }
    }

    pub(crate) fn close_event_stream(&self, session_id: SessionID) {
        // to-do: Make sure the following logic is correct.
        // No need to remove the display handler, because it will be removed when the connection is closed.
        if let Some(session) = self.session_handlers.write().unwrap().get_mut(&session_id) {
            try_send_close_event(&session.event_stream);
        }
    }

    pub(super) fn make_displays_msg(displays: &Vec<DisplayInfo>) -> String {
        let mut msg_vec = Vec::new();
        for ref d in displays.iter() {
            let mut h: HashMap<&str, i32> = Default::default();
            h.insert("x", d.x);
            h.insert("y", d.y);
            h.insert("width", d.width);
            h.insert("height", d.height);
            h.insert("cursor_embedded", if d.cursor_embedded { 1 } else { 0 });
            if let Some(original_resolution) = d.original_resolution.as_ref() {
                h.insert("original_width", original_resolution.width);
                h.insert("original_height", original_resolution.height);
            }
            // Don't convert scale (x 100) to i32 directly.
            // (d.scale * 100.0f64) as i32 may produces inaccuracies.
            //
            // Example: GNOME Wayland with Fractional Scaling enabled:
            // - Physical resolution: 2560x1600
            // - Logical resolution: 1074x1065
            // - Scale factor: 150%
            // Passing physical dimensions and scale factor prevents accurate logical resolution calculation
            // since 2560/1.5 = 1706.666... (rounded to 1706.67) and 1600/1.5 = 1066.666... (rounded to 1066.67)
            // h.insert("scale", (d.scale * 100.0f64) as i32);

            // Send scaled_width for accurate logical scale calculation.
            if d.scale > 0.0 {
                let scaled_width = (d.width as f64 / d.scale).round() as i32;
                h.insert("scaled_width", scaled_width);
            }
            msg_vec.push(h);
        }
        serde_json::ser::to_string(&msg_vec).unwrap_or("".to_owned())
    }

    pub fn update_use_texture_render(&self) {
        self.use_texture_render
            .store(crate::ui_interface::use_texture_render(), Ordering::Relaxed);
        self.display_rgbas.write().unwrap().clear();
    }
}

impl FlutterHandler {
    #[inline]
    pub(super) fn on_rgba_soft_render(&self, display: usize, rgba: &mut scrap::ImageRgb) {
        // If the current rgba is not fetched by flutter, i.e., is valid.
        // We give up sending a new event to flutter.
        let mut rgba_write_lock = self.display_rgbas.write().unwrap();
        if let Some(rgba_data) = rgba_write_lock.get_mut(&display) {
            if rgba_data.valid {
                return;
            } else {
                rgba_data.valid = true;
            }
            // Return the rgba buffer to the video handler for reusing allocated rgba buffer.
            std::mem::swap::<Vec<u8>>(&mut rgba.raw, &mut rgba_data.data);
        } else {
            let mut rgba_data = RgbaData::default();
            std::mem::swap::<Vec<u8>>(&mut rgba.raw, &mut rgba_data.data);
            rgba_data.valid = true;
            rgba_write_lock.insert(display, rgba_data);
        }
        drop(rgba_write_lock);

        let mut is_sent = false;
        let is_multi_sessions = self.is_multi_ui_session();
        for h in self.session_handlers.read().unwrap().values() {
            // The soft renderer does not support multi-displays session for now.
            if h.displays.len() > 1 {
                continue;
            }
            // If there're multiple ui sessions, we only notify the ui session that has the display.
            if is_multi_sessions {
                if !h.displays.contains(&display) {
                    continue;
                }
            }
            if let Some(stream) = &h.event_stream {
                stream.add(EventToUI::Rgba(display));
                is_sent = true;
            }
        }
        // We need `is_sent` here. Because we use texture render for multi-displays session.
        //
        // Eg. We have two windows, one is display 1, the other is displays 0&1.
        // When image of display 0 is received, we will not send the event.
        //
        // 1. "display 1" will not send the event.
        // 2. "displays 0&1" will not send the event. Because it uses texutre render for now.
        if !is_sent {
            if let Some(rgba_data) = self.display_rgbas.write().unwrap().get_mut(&display) {
                rgba_data.valid = false;
            }
        }
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn on_rgba_flutter_texture_render(
        &self,
        use_texture_render: bool,
        display: usize,
        rgba: &mut scrap::ImageRgb,
    ) {
        for (_, session) in self.session_handlers.read().unwrap().iter() {
            if use_texture_render || session.displays.len() > 1 {
                if session.renderer.on_rgba(display, rgba) {
                    if let Some(stream) = &session.event_stream {
                        stream.add(EventToUI::Texture(display, false));
                    }
                }
            }
        }
    }
}

#[inline]
pub(super) fn serialize_resolutions(resolutions: &Vec<Resolution>) -> String {
    #[derive(Debug, serde::Serialize)]
    struct ResolutionSerde {
        width: i32,
        height: i32,
    }

    let mut v = vec![];
    resolutions
        .iter()
        .map(|r| {
            v.push(ResolutionSerde {
                width: r.width,
                height: r.height,
            })
        })
        .count();
    serde_json::ser::to_string(&v).unwrap_or("".to_string())
}
