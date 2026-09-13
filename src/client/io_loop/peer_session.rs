use super::*;

impl<T: InvokeUiSession> Remote<T> {
    pub(super) fn handle_message_box(&mut self, msgbox: MessageBox) {
        let mut link = msgbox.link;
        if let Some(v) = config::HELPER_URL.get(&link as &str) {
            link = v.to_string();
        } else {
            log::warn!("Message box ignore link {} for security", &link);
            link = "".to_string();
        }
        self.handler
            .msgbox(&msgbox.msgtype, &msgbox.title, &msgbox.text, &link);
    }

    pub(super) fn handle_voice_call_request(&mut self, request: VoiceCallRequest) {
        if request.is_connect {
            // TODO: maybe we will do a voice call from the peer in the future.
        } else {
            log::debug!("The remote has requested to close the voice call");
            if let Some(sender) = self.stop_voice_call_sender.take() {
                allow_err!(sender.send(()));
                self.handler.on_voice_call_closed("");
            }
        }
    }

    pub(super) fn handle_voice_call_response(&mut self, response: VoiceCallResponse) {
        let ts = std::mem::replace(&mut self.voice_call_request_timestamp, None);
        if let Some(ts) = ts {
            if response.req_timestamp != ts.get() {
                log::debug!("Possible encountering a voice call attack.");
            } else {
                if response.accepted {
                    // The peer accepted the voice call.
                    self.handler.on_voice_call_started();
                    self.stop_voice_call_sender = self.start_voice_call();
                } else {
                    // The peer refused the voice call.
                    self.handler.on_voice_call_closed("");
                }
            }
        }
    }

    pub(super) fn handle_bare_peer_info(&mut self, pi: PeerInfo) {
        self.handler.set_displays(&pi.displays);
        self.handler.set_platform_additions(&pi.platform_additions);
    }

    pub(super) fn handle_screenshot_response(&mut self, response: ScreenshotResponse) {
        crate::client::screenshot::set_screenshot(response.data);
        self.handler
            .handle_screenshot_resp(response.sid, response.msg);
    }

    pub(super) fn handle_terminal_response(&mut self, response: TerminalResponse) {
        use base::message_proto::terminal_response::Union;
        if let Some(Union::Opened(opened)) = &response.union {
            if opened.success && !opened.service_id.is_empty() {
                let mut lc = self.handler.lc.write().unwrap();
                let key = lc.get_key_terminal_service_id().to_owned();
                lc.set_option(key, opened.service_id.clone());
            }
        }
        self.handler.handle_terminal_response(response);
    }
}
