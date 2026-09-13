use super::*;

impl Connection {
    pub(super) fn handle_audio_frame(&mut self, frame: AudioFrame) {
        if !self.disable_audio {
            if let Some(sender) = &self.audio_sender {
                allow_err!(sender.send(MediaData::AudioFrame(Box::new(frame))));
            } else {
                log::warn!(
                    "Processing audio frame without the voice call audio sender."
                );
            }
        }
    }

    pub(super) async fn handle_voice_call_request(&mut self, request: VoiceCallRequest) {
        if request.is_connect {
            self.voice_call_request_timestamp = Some(
                NonZeroI64::new(request.req_timestamp)
                    .unwrap_or(NonZeroI64::new(get_time()).unwrap()),
            );
            // Notify the connection manager.
            self.send_to_cm(Data::VoiceCallIncoming);
        } else {
            self.close_voice_call().await;
        }
    }

    pub(super) fn handle_screenshot_request(&mut self, request: ScreenshotRequest) {
        if let Some(tx) = self.inner.tx.clone() {
            crate::video_service::set_take_screenshot(
                self.video_source(),
                request.display as _,
                request.sid.clone(),
                tx,
            );
            self.refresh_video_display(Some(request.display as usize));
        }
    }

    pub(super) async fn handle_terminal_action_msg(&mut self, action: TerminalAction) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        allow_err!(self.handle_terminal_action(action).await);
        #[cfg(any(target_os = "android", target_os = "ios"))]
        log::warn!("Terminal action received but not supported on this platform");
    }
}
