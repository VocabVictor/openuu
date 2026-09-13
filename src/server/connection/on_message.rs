use super::*;

impl Connection {
    pub(super) async fn on_message(&mut self, msg: Message) -> bool {
        if let Some(message::Union::Misc(misc)) = &msg.union {
            // Move the CloseReason forward, as this message needs to be received when unauthorized, especially for kcp.
            if let Some(misc::Union::CloseReason(s)) = &misc.union {
                log::info!("receive close reason: {}", s);
                self.on_close("Peer close", true).await;
                raii::AuthedConnID::check_remove_session(self.inner.id(), self.session_key());
                return false;
            }
        }
        if self.authorized {
            if matches!(msg.union.as_ref(), Some(message::Union::LoginRequest(_))) {
                return true;
            }
            if let Some(message) = self.authorized_scope_violation(&msg) {
                return self.handle_authorized_scope_violation(message).await;
            }
        }
        // After handling CloseReason messages, proceed to process other message types
        if let Some(message::Union::LoginRequest(lr)) = msg.union {
            return self.handle_login_request(lr).await;
        } else if let Some(message::Union::Auth2fa(tfa)) = msg.union {
            return self.handle_auth_2fa(tfa).await;
        } else if let Some(message::Union::TestDelay(t)) = msg.union {
            self.handle_test_delay(t);
        } else if let Some(message::Union::SwitchSidesResponse(s)) = msg.union {
            return self.handle_switch_sides_response(s).await;
        } else if self.authorized {
            if self.port_forward_socket.is_some() {
                return true;
            }
            match msg.union {
                Some(message::Union::MouseEvent(me)) => self.handle_mouse_event(me),
                Some(message::Union::PointerDeviceEvent(pde)) => self.handle_pointer_device_event(pde),
                #[cfg(any(target_os = "ios"))]
                Some(message::Union::KeyEvent(..)) => {}
                #[cfg(any(target_os = "android"))]
                Some(message::Union::KeyEvent(me)) => self.handle_key_event(me),
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                Some(message::Union::KeyEvent(me)) => self.handle_key_event(me),
                Some(message::Union::Clipboard(cb)) => self.handle_clipboard(cb),
                Some(message::Union::MultiClipboards(_mcb)) => self.handle_multi_clipboards(_mcb),
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                Some(message::Union::Cliprdr(clip)) => self.handle_cliprdr(clip).await,
                Some(message::Union::FileAction(fa)) => self.handle_file_action(fa).await,
                Some(message::Union::FileResponse(fr)) => self.handle_file_response(fr),
                Some(message::Union::Misc(misc)) => return self.handle_misc(misc).await,
                Some(message::Union::AudioFrame(frame)) => self.handle_audio_frame(frame),
                Some(message::Union::VoiceCallRequest(request)) => self.handle_voice_call_request(request).await,
                Some(message::Union::VoiceCallResponse(_response)) => {
                    // TODO: Maybe we can do a voice call from cm directly.
                }
                Some(message::Union::ScreenshotRequest(request)) => self.handle_screenshot_request(request),
                Some(message::Union::PortForwardChannel(ch)) => self.handle_port_forward_channel(ch),
                Some(message::Union::TerminalAction(action)) => self.handle_terminal_action_msg(action).await,
                _ => {}
            }
        }
        true
    }
}
