use super::*;

impl Connection {
    /// `Misc`: display, option, chat, audio-format and session-control
    /// requests of an authorized session. Returns false when the connection
    /// must close (switch sides, session selection).
    pub(super) async fn handle_misc(&mut self, misc: Misc) -> bool {
        match misc.union {
            Some(misc::Union::SwitchDisplay(s)) => {
                self.handle_switch_display(s).await;
            }
            Some(misc::Union::CaptureDisplays(displays)) => {
                let add = displays.add.iter().map(|d| *d as usize).collect::<Vec<_>>();
                let sub = displays.sub.iter().map(|d| *d as usize).collect::<Vec<_>>();
                let set = displays.set.iter().map(|d| *d as usize).collect::<Vec<_>>();
                self.capture_displays(&add, &sub, &set).await;
            }
            #[cfg(windows)]
            Some(misc::Union::ToggleVirtualDisplay(t)) => {
                if !self.view_camera {
                    self.toggle_virtual_display(t).await;
                }
            }
            Some(misc::Union::TogglePrivacyMode(t)) => {
                if !self.view_camera {
                    self.toggle_privacy_mode(t).await;
                }
            }
            Some(misc::Union::ChatMessage(c)) => {
                self.send_to_cm(ipc::Data::ChatMessage { text: c.text });
                self.chat_unanswered = true;
                self.update_auto_disconnect_timer();
            }
            Some(misc::Union::Option(o)) => {
                if self.authed_conn_type() == Some(AuthConnType::Remote) {
                    self.update_options(&o).await;
                } else if let Some(option) = self.scoped_update_option_message(&o) {
                    self.update_options(&option).await;
                }
            }
            Some(misc::Union::RefreshVideo(r)) => {
                if self.should_handle_render_broadcast_message() {
                    if r {
                        // Refresh all videos.
                        // Compatibility with old versions and sciter(remote).
                        self.refresh_video_display(None);
                    }
                    self.update_auto_disconnect_timer();
                }
            }
            Some(misc::Union::RefreshVideoDisplay(display)) => {
                if self.should_handle_render_broadcast_message() {
                    self.refresh_video_display(Some(display as usize));
                    self.update_auto_disconnect_timer();
                }
            }
            Some(misc::Union::VideoReceived(_)) => {
                video_service::notify_video_frame_fetched_by_conn_id(
                    self.inner.id,
                    Some(Instant::now().into()),
                );
            }
            Some(misc::Union::QuickLaunchRequest(request)) => {
                let denied = crate::quick_launch::denied(&request);
                let response = if self.authorized && self.is_authed_remote_conn() && self.peer_keyboard_enabled() {
                    match hbb_common::tokio::task::spawn_blocking(move || crate::quick_launch::handle(&request)).await {
                        Ok(response) => response,
                        Err(error) => { log::error!("Quick launch worker failed: {error}"); denied }
                    }
                } else { denied };
                let mut misc = Misc::new();
                misc.set_quick_launch_response(response);
                let mut msg = Message::new();
                msg.set_misc(misc);
                self.send(msg).await;
            }
            Some(misc::Union::RestartRemoteDevice(_)) => {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                if self.restart {
                    // force_reboot, not work on linux vm and macos 14
                    #[cfg(any(target_os = "linux", target_os = "windows"))]
                    match system_shutdown::force_reboot() {
                        Ok(_) => log::info!("Restart by the peer"),
                        Err(e) => log::error!("Failed to restart: {}", e),
                    }
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    match system_shutdown::reboot() {
                        Ok(_) => log::info!("Restart by the peer"),
                        Err(e) => log::error!("Failed to restart: {}", e),
                    }
                }
            }
            #[cfg(windows)]
            Some(misc::Union::ElevationRequest(r)) => match r.union {
                Some(elevation_request::Union::Direct(_)) => {
                    self.handle_elevation_request(portable_client::StartPara::Direct)
                        .await;
                }
                Some(elevation_request::Union::Logon(r)) => {
                    self.handle_elevation_request(portable_client::StartPara::Logon(
                        r.username, r.password,
                    ))
                    .await;
                }
                _ => {}
            },
            Some(misc::Union::AudioFormat(format)) => {
                if !self.disable_audio {
                    // Drop the audio sender previously.
                    drop(std::mem::replace(&mut self.audio_sender, None));
                    self.audio_sender = Some(start_audio_thread());
                    self.audio_sender
                        .as_ref()
                        .map(|a| allow_err!(a.send(MediaData::AudioFormat(format))));
                }
            }
            #[cfg(feature = "flutter")]
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            Some(misc::Union::SwitchSidesRequest(s)) => {
                if let Ok(uuid) = uuid::Uuid::from_slice(&s.uuid.to_vec()[..]) {
                    if crate::server::insert_pending_switch_sides_uuid(
                        self.lr.my_id.clone(),
                        uuid.clone(),
                    ) {
                        crate::run_me(vec![
                            "--connect",
                            &self.lr.my_id,
                            "--switch_uuid",
                            uuid.to_string().as_ref(),
                        ])
                        .ok();
                    }
                    self.on_close("switch sides", false).await;
                    return false;
                }
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            Some(misc::Union::ChangeResolution(r)) => {
                if !self.view_camera {
                    self.change_resolution(None, &r);
                }
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            Some(misc::Union::ChangeDisplayResolution(dr)) => {
                if !self.view_camera {
                    self.change_resolution(Some(dr.display as _), &dr.resolution);
                }
            }
            Some(misc::Union::AutoAdjustFps(fps)) => video_service::VIDEO_QOS
                .lock()
                .unwrap()
                .user_auto_adjust_fps(self.inner.id(), fps),
            Some(misc::Union::ClientRecordStatus(status)) => video_service::VIDEO_QOS
                .lock()
                .unwrap()
                .user_record(self.inner.id(), status),
            #[cfg(windows)]
            Some(misc::Union::SelectedSid(sid)) => {
                if let Some(current_process_sid) =
                    crate::platform::get_current_process_session_id()
                {
                    let sessions = crate::platform::get_available_sessions(false);
                    crate::platform::windows::sessions::pin_session_from_selection(
                        sid, &sessions,
                    );
                    if crate::platform::is_installed()
                        && crate::platform::is_share_rdp()
                        && raii::AuthedConnID::non_port_forward_conn_count() == 1
                        && sessions.len() > 1
                        && current_process_sid != sid
                        && sessions.iter().any(|e| e.sid == sid)
                    {
                        std::thread::spawn(move || {
                            let _ = ipc::connect_to_user_session(Some(sid));
                        });
                        return false;
                    }
                    if self.file_transfer.is_some() {
                        if let Some((dir, show_hidden)) = self.delayed_read_dir.take() {
                            self.read_dir(&dir, show_hidden);
                        }
                    } else if self.view_camera {
                        self.try_sub_camera_displays();
                    } else if !self.terminal {
                        self.try_sub_monitor_services();
                    }
                }
            }
            Some(misc::Union::MessageQuery(mq)) => {
                if let Some(msg_out) = video_service::make_display_changed_msg(
                    mq.switch_display as _,
                    None,
                    self.video_source(),
                ) {
                    self.send(msg_out).await;
                }
            }
            _ => {}
        }
        true
    }
}
