use super::*;

impl Connection {
    pub(super) fn refresh_video_display(&self, display: Option<usize>) {
        video_service::refresh();
        self.server.upgrade().map(|s| {
            s.read().unwrap().set_video_service_opt(
                display.map(|d| (self.video_source(), d)),
                video_service::OPTION_REFRESH,
                super::super::service::SERVICE_OPTION_VALUE_TRUE,
            );
        });
    }

    pub(super) async fn handle_switch_display(&mut self, s: SwitchDisplay) {
        let display_idx = s.display as usize;
        if self.display_idx != display_idx {
            if let Some(server) = self.server.upgrade() {
                if !self.switch_display_to(display_idx, server.clone()) {
                    return;
                }

                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                if !self.view_camera && s.width != 0 && s.height != 0 {
                    self.change_resolution(
                        None,
                        &Resolution {
                            width: s.width,
                            height: s.height,
                            ..Default::default()
                        },
                    );
                }
            }

            // Send display changed message.
            // 1. For compatibility with old versions ( < 1.2.4 ).
            // 2. Sciter version.
            // 3. Update `SupportedResolutions`.
            if let Some(msg_out) =
                video_service::make_display_changed_msg(self.display_idx, None, self.video_source())
            {
                self.send(msg_out).await;
            }
        }
    }

    pub(super) fn video_source_count(video_source: VideoSource) -> usize {
        match video_source {
            VideoSource::Monitor => display_service::get_sync_displays().len(),
            VideoSource::Camera => camera::Cameras::get_sync_cameras().len(),
        }
    }

    pub(super) fn video_source(&self) -> VideoSource {
        if self.view_camera {
            VideoSource::Camera
        } else {
            VideoSource::Monitor
        }
    }

    pub(super) fn switch_display_to(&mut self, display_idx: usize, server: Arc<RwLock<Server>>) -> bool {
        let source_count = Self::video_source_count(self.video_source());
        if display_idx >= source_count {
            // Do not remap an explicit switch: its resolution belongs to the requested source.
            log::warn!(
                "Ignore switch to invalid {:?} index {}, available source count: {}",
                self.video_source(),
                display_idx,
                source_count
            );
            return false;
        }

        let new_service_name = video_service::get_service_name(self.video_source(), display_idx);
        let old_service_name =
            video_service::get_service_name(self.video_source(), self.display_idx);
        let mut lock = server.write().unwrap();
        if !lock.contains(&new_service_name) {
            lock.add_service(Box::new(video_service::new(
                self.video_source(),
                display_idx,
            )));
        }
        // For versions greater than 1.2.4, a `CaptureDisplays` message will be sent immediately.
        // Unnecessary capturers will be removed then.
        if !crate::common::is_support_multi_ui_session(&self.lr.version) {
            lock.subscribe(&old_service_name, self.inner.clone(), false);
        }
        lock.subscribe(&new_service_name, self.inner.clone(), true);
        self.display_idx = display_idx;
        true
    }

    #[cfg(windows)]
    pub(super) async fn handle_elevation_request(&mut self, para: portable_client::StartPara) {
        let mut err;
        if !self.keyboard {
            err = "No permission".to_string();
        } else {
            err = "No need to elevate".to_string();
            if !crate::platform::is_installed() && !portable_client::running() {
                err = portable_client::start_portable_service(para)
                    .err()
                    .map_or("".to_string(), |e| e.to_string());
            }
        }

        let mut misc = Misc::new();
        misc.set_elevation_response(err);
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(msg).await;
        self.update_auto_disconnect_timer();
    }

    pub(super) async fn capture_displays(&mut self, add: &[usize], sub: &[usize], set: &[usize]) {
        let video_source = self.video_source();
        let source_count = Self::video_source_count(video_source);
        // Only add/set can create services; sub only narrows existing subscriptions.
        let valid_add = add
            .iter()
            .copied()
            .filter(|display| *display < source_count)
            .collect::<Vec<_>>();
        let valid_sub = sub
            .iter()
            .copied()
            .filter(|display| *display < source_count)
            .collect::<Vec<_>>();
        let valid_set = set
            .iter()
            .copied()
            .filter(|display| *display < source_count)
            .collect::<Vec<_>>();
        let invalid_count =
            add.len() + sub.len() + set.len() - valid_add.len() - valid_sub.len() - valid_set.len();
        if invalid_count != 0 {
            log::warn!(
                "Ignore {} invalid {:?} indices, available source count: {}",
                invalid_count,
                video_source,
                source_count
            );
        }
        // Passing an invalid sub request as an empty exclude list would unsubscribe all services.
        if (!add.is_empty() && valid_add.is_empty())
            || (add.is_empty() && !sub.is_empty() && valid_sub.is_empty())
            || (add.is_empty() && sub.is_empty() && !set.is_empty() && valid_set.is_empty())
        {
            return;
        }

        if let Some(server) = self.server.upgrade() {
            let mut lock = server.write().unwrap();
            for display in valid_add.iter() {
                let service_name = video_service::get_service_name(video_source, *display);
                if !lock.contains(&service_name) {
                    lock.add_service(Box::new(video_service::new(video_source, *display)));
                }
            }
            for display in valid_set.iter() {
                let service_name = video_service::get_service_name(video_source, *display);
                if !lock.contains(&service_name) {
                    lock.add_service(Box::new(video_service::new(video_source, *display)));
                }
            }
            if !add.is_empty() {
                lock.capture_displays(self.inner.clone(), video_source, &valid_add, true, false);
            } else if !sub.is_empty() {
                lock.capture_displays(self.inner.clone(), video_source, &valid_sub, false, true);
            } else {
                lock.capture_displays(self.inner.clone(), video_source, &valid_set, true, true);
            }
            self.multi_ui_session = lock.get_subbed_displays_count(self.inner.id()) > 1;
            if self.follow_remote_window {
                lock.subscribe(
                    NAME_WINDOW_FOCUS,
                    self.inner.clone(),
                    !self.multi_ui_session,
                );
            }
            drop(lock);
        }
    }

    #[cfg(windows)]
    pub(super) async fn toggle_virtual_display(&mut self, t: ToggleVirtualDisplay) {
        let make_msg = |text: String| {
            let mut msg_out = Message::new();
            let res = MessageBox {
                msgtype: "nook-nocancel-hasclose".to_owned(),
                title: "Virtual display".to_owned(),
                text,
                link: "".to_owned(),
                ..Default::default()
            };
            msg_out.set_message_box(res);
            msg_out
        };

        if t.on {
            if !virtual_display_manager::is_virtual_display_supported() {
                self.send(make_msg("idd_not_support_under_win10_2004_tip".to_string()))
                    .await;
            } else {
                if let Err(e) = virtual_display_manager::plug_in_monitor(t.display as _, Vec::new())
                {
                    log::error!("Failed to plug in virtual display: {}", e);
                    self.send(make_msg(format!(
                        "Failed to plug in virtual display: {}",
                        e
                    )))
                    .await;
                }
            }
        } else {
            if let Err(e) = virtual_display_manager::plug_out_monitor(t.display, false, true) {
                log::error!("Failed to plug out virtual display {}: {}", t.display, e);
                self.send(make_msg(format!(
                    "Failed to plug out virtual displays: {}",
                    e
                )))
                .await;
            }
        }
    }

    pub(super) async fn toggle_privacy_mode(&mut self, t: TogglePrivacyMode) {
        if t.on {
            self.turn_on_privacy(t.impl_key).await;
        } else {
            self.turn_off_privacy(t.impl_key).await;
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn change_resolution(&mut self, d: Option<usize>, r: &Resolution) {
        if self.keyboard {
            if let Ok(displays) = display_service::try_get_displays() {
                let display_idx = d.unwrap_or(self.display_idx);
                if let Some(display) = displays.get(display_idx) {
                    let name = display.name();
                    #[cfg(windows)]
                    if let Some(_ok) =
                        virtual_display_manager::rustdesk_idd::change_resolution_if_is_virtual_display(
                            &name,
                            r.width as _,
                            r.height as _,
                        )
                    {
                        return;
                    }
                    #[allow(unused_mut)]
                    let mut record_changed = true;
                    #[cfg(windows)]
                    if virtual_display_manager::amyuni_idd::is_my_display(&name) {
                        record_changed = false;
                    }
                    #[cfg(not(target_os = "macos"))]
                    let scale = 1.0;
                    #[cfg(target_os = "macos")]
                    let scale = display.scale();
                    let original = (
                        ((display.width() as f64) / scale).round() as _,
                        (display.height() as f64 / scale).round() as _,
                    );
                    if record_changed {
                        display_service::set_last_changed_resolution(
                            &name,
                            original,
                            (r.width, r.height),
                        );
                    }
                    if let Err(e) =
                        crate::platform::change_resolution(&name, r.width as _, r.height as _)
                    {
                        log::error!(
                            "Failed to change resolution '{}' to ({},{}): {:?}",
                            &name,
                            r.width,
                            r.height,
                            e
                        );
                    }
                }
            }
        }
    }
}
