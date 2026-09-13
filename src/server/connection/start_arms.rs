use super::*;

impl Connection {
    /// Send one queued message to the peer; `false` ends the loop.
    pub(super) async fn send_queued(&mut self, instant: Instant, value: Arc<Message>) -> bool {
        let latency = instant.elapsed().as_millis() as i64;
        #[allow(unused_mut)]
        let mut msg = value;

        if latency > 1000 {
            match &msg.union {
                Some(message::Union::AudioFrame(_)) => {
                    // log::info!("audio frame latency {}", instant.elapsed().as_secs_f32());
                    return true;
                }
                _ => {}
            }
        }
        match &msg.union {
            Some(message::Union::Misc(m)) => {
                match &m.union {
                    Some(misc::Union::StopService(_)) => {
                        self.send_close_reason_no_retry("").await;
                        self.on_close("stop service", false).await;
                        return false;
                    }
                    _ => {},
                }
            }
            Some(message::Union::PeerInfo(_pi)) => {
                self.refresh_video_display(None);
                #[cfg(target_os = "macos")]
                self.retina.set_displays(&_pi.displays);
            }
            Some(message::Union::CursorPosition(pos)) => {
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    if self.follow_remote_cursor {
                        self.handle_cursor_switch_display(pos.clone()).await;
                    }
                }
                #[cfg(target_os = "macos")]
                if let Some(new_msg) = self.retina.on_cursor_pos(&pos, self.display_idx) {
                    msg = Arc::new(new_msg);
                }
            }
            Some(message::Union::MultiClipboards(_multi_clipboards)) => {
                #[cfg(not(target_os = "ios"))]
                if let Some(msg_out) = crate::clipboard::get_msg_if_not_support_multi_clip(&self.lr.version, &self.lr.my_platform, _multi_clipboards) {
                    if let Err(err) = self.stream.send(&msg_out).await {
                        self.on_close(&err.to_string(), false).await;
                        return false;
                    }
                    return true;
                }
            }
            _ => {}
        }

        let msg: &Message = &msg;
        if let Err(err) = self.stream.send(msg).await {
            self.on_close(&err.to_string(), false).await;
            return false;
        }
        true
    }

    /// Once-a-second housekeeping; `false` ends the loop.
    pub(super) async fn on_second_tick(&mut self, queued_video: usize) -> bool {
        let id = self.inner.id();
        #[cfg(windows)]
        self.portable_check();
        raii::AuthedConnID::check_wake_lock_on_setting_changed();
        if let Some((instant, minute)) = self.auto_disconnect_timer.as_ref() {
            if instant.elapsed().as_secs() > minute * 60 {
                self.send_close_reason_no_retry("Connection failed due to inactivity").await;
                self.on_close("auto disconnect", true).await;
                return false;
            }
        }
        if video_service::qos_diag_verbose() && self.video_send_count > 0 {
            // Joined with `qos_trace` on `t`: a probe that waits behind a
            // blocked write is not a slow network.
            log::debug!(
                "qos_send t={} id={id} frames={} send_max={} send_sum={} queued={}",
                hbb_common::get_time(),
                self.video_send_count,
                self.video_send_max_ms,
                self.video_send_sum_ms,
                queued_video
            );
            self.video_send_max_ms = 0;
            self.video_send_sum_ms = 0;
            self.video_send_count = 0;
        }
        self.file_remove_log_control.on_timer().drain(..).map(|x| self.send_to_cm(x)).count();
        #[cfg(feature = "hwcodec")]
        self.update_supported_encoding();
        true
    }

    /// Probe the peer and enforce the receive timeout; `false` ends the loop.
    pub(super) async fn on_test_delay_tick(&mut self, last_recv_time: Instant) -> bool {
        let id = self.inner.id();
        if last_recv_time.elapsed() >= SEC30 {
            self.on_close("Timeout", true).await;
            return false;
        }
        // The control end will jump out of the loop after receiving LoginResponse and will not reply to the TestDelay
        if self.last_test_delay.is_none() && !(self.port_forward_socket.is_some() && self.authorized) {
            self.last_test_delay = Some(Instant::now());
            let mut msg_out = Message::new();
            msg_out.set_test_delay(TestDelay{
                last_delay: self.network_delay,
                target_bitrate: video_service::VIDEO_QOS.lock().unwrap().bitrate(),
                ..Default::default()
            });
            self.send(msg_out.into()).await;
        }
        if self.is_authed_remote_conn() || self.view_camera {
            if let Some(last_test_delay) = self.last_test_delay {
                video_service::VIDEO_QOS.lock().unwrap().user_delay_response_elapsed(id, last_test_delay.elapsed().as_millis());
            }
        }
        true
    }
}
