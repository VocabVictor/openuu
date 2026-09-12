use super::*;

impl Connection {
    pub(super) async fn turn_on_privacy(&mut self, impl_key: String) {
        if !self.is_authed_remote_conn() || !self.privacy_mode {
            let msg_out = crate::common::make_privacy_mode_msg(
                back_notification::PrivacyModeState::PrvOnFailedDenied,
                impl_key,
            );
            self.send(msg_out).await;
            return;
        }

        let msg_out = if !privacy_mode::is_privacy_mode_supported() {
            crate::common::make_privacy_mode_msg_with_details(
                back_notification::PrivacyModeState::PrvNotSupported,
                "Unsupported. 1 Multi-screen is not supported. 2 Please confirm the license is activated.".to_string(),
                impl_key,
            )
        } else {
            let is_pre_privacy_on = privacy_mode::is_in_privacy_mode();
            let pre_impl_key = privacy_mode::get_cur_impl_key();

            if is_pre_privacy_on {
                if let Some(pre_impl_key) = pre_impl_key {
                    if !privacy_mode::is_current_privacy_mode_impl(&pre_impl_key) {
                        let off_msg = crate::common::make_privacy_mode_msg(
                            back_notification::PrivacyModeState::PrvOffSucceeded,
                            pre_impl_key,
                        );
                        self.send(off_msg).await;
                    }
                }
            }

            let turn_on_res = privacy_mode::turn_on_privacy(&impl_key, self.inner.id).await;
            match turn_on_res {
                Some(Ok(res)) => {
                    if res {
                        let err_msg = privacy_mode::check_privacy_mode_err(
                            self.inner.id,
                            self.display_idx,
                            5_000,
                        );
                        if err_msg.is_empty() {
                            crate::common::make_privacy_mode_msg(
                                back_notification::PrivacyModeState::PrvOnSucceeded,
                                impl_key,
                            )
                        } else {
                            log::error!(
                                "Check privacy mode failed: {}, turn off privacy mode.",
                                &err_msg
                            );
                            let _ = Self::turn_off_privacy_to_msg(self.inner.id, String::new());
                            crate::common::make_privacy_mode_msg_with_details(
                                back_notification::PrivacyModeState::PrvOnFailed,
                                err_msg,
                                impl_key,
                            )
                        }
                    } else {
                        crate::common::make_privacy_mode_msg(
                            back_notification::PrivacyModeState::PrvOnFailed,
                            impl_key,
                        )
                    }
                }
                Some(Err(e)) => {
                    log::error!("Failed to turn on privacy mode. {}", e);
                    if privacy_mode::is_in_privacy_mode() {
                        let _ = Self::turn_off_privacy_to_msg(
                            privacy_mode::INVALID_PRIVACY_MODE_CONN_ID,
                            String::new(),
                        );
                    }
                    crate::common::make_privacy_mode_msg_with_details(
                        back_notification::PrivacyModeState::PrvOnFailed,
                        e.to_string(),
                        impl_key,
                    )
                }
                None => crate::common::make_privacy_mode_msg_with_details(
                    back_notification::PrivacyModeState::PrvOffFailed,
                    "Not supported".to_string(),
                    impl_key,
                ),
            }
        };
        self.send(msg_out).await;
    }

    pub(super) async fn turn_off_privacy(&mut self, impl_key: String) {
        let msg_out = if !privacy_mode::is_privacy_mode_supported() {
            crate::common::make_privacy_mode_msg_with_details(
                back_notification::PrivacyModeState::PrvNotSupported,
                // This error message is used for magnifier. It is ok to use it here.
                "Unsupported. 1 Multi-screen is not supported. 2 Please confirm the license is activated.".to_string(),
                impl_key,
            )
        } else {
            Self::turn_off_privacy_to_msg(self.inner.id, impl_key)
        };
        self.send(msg_out).await;
    }

    pub fn turn_off_privacy_to_msg(_conn_id: i32, impl_key: String) -> Message {
        Self::turn_off_privacy_result_to_msg(
            privacy_mode::turn_off_privacy(_conn_id, None),
            impl_key,
        )
    }

    pub(super) fn turn_off_privacy_result_to_msg(
        turn_off_res: Option<hbb_common::ResultType<()>>,
        impl_key: String,
    ) -> Message {
        match turn_off_res {
            Some(Ok(_)) => crate::common::make_privacy_mode_msg(
                back_notification::PrivacyModeState::PrvOffSucceeded,
                impl_key,
            ),
            Some(Err(e)) => {
                log::error!("Failed to turn off privacy mode {}", e);
                crate::common::make_privacy_mode_msg_with_details(
                    back_notification::PrivacyModeState::PrvOffFailed,
                    e.to_string(),
                    impl_key,
                )
            }
            None => crate::common::make_privacy_mode_msg_with_details(
                back_notification::PrivacyModeState::PrvOffFailed,
                "Not supported".to_string(),
                impl_key,
            ),
        }
    }

    pub(super) async fn on_close(&mut self, reason: &str, lock: bool) {
        if self.closed {
            return;
        }
        self.closed = true;
        // If voice A,B -> C, and A,B has voice call
        // B disconnects, C will reset the voice call input.
        //
        // It may be acceptable, because it's not a common case,
        // and it's immediately known when the input device changes.
        // C can change the input device manually in cm interface.
        //
        // We can add a (Vec<conn_id>, input device) to avoid this.
        // But it's not necessary now and we have to consider two audio services(client, server).
        crate::audio_service::set_voice_call_input_device(None, true);
        log::info!("#{} Connection closed: {}", self.inner.id(), reason);
        if lock
            && self.lock_after_session_end
            && self.keyboard
            && !raii::AuthedConnID::session_reconnected(self.inner.id(), &self.session_key())
        {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            lock_screen().await;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let data = if self.chat_unanswered || self.file_transferred && cfg!(feature = "flutter") {
            ipc::Data::Disconnected
        } else {
            ipc::Data::Close
        };
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let data = ipc::Data::Close;
        self.tx_to_cm.send(data).ok();
        self.port_forward_socket.take();
        if let Some(mut mux) = self.port_forward_mux.take() {
            mux.close_all();
        }
    }

    // The `reason` should be consistent with `check_if_retry` if not empty
    pub(super) async fn send_close_reason_no_retry(&mut self, reason: &str) {
        let mut misc = Misc::new();
        if reason.is_empty() {
            misc.set_close_reason("Closed manually by the peer".to_string());
        } else {
            misc.set_close_reason(reason.to_string());
        }
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(msg_out).await;
        raii::AuthedConnID::check_remove_session(self.inner.id(), self.session_key());
    }
}
