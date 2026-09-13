use super::*;

impl Connection {
    /// Handle one message from the connection manager; `false` ends the loop.
    pub(super) async fn handle_cm_data(&mut self, data: ipc::Data) -> bool {
        match data {
            ipc::Data::Authorize => {
                self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::Click);
                self.require_2fa.take();
                if !self.send_logon_response_and_keep_alive().await {
                    return false;
                }
                if self.port_forward_socket.is_some() {
                    return false;
                }
            }
            ipc::Data::Close => {
                self.chat_unanswered = false; // seen
                self.file_transferred = false; //seen
                self.send_close_reason_no_retry("").await;
                self.on_close("connection manager", true).await;
                return false;
            }
            // The connection manager's window went away rather than a person
            // disconnecting this peer. End the session exactly as above, but do not
            // send the manual close reason: it is the one thing that stops the peer
            // from retrying, and on a logout the retry is the whole point - it is
            // what puts the peer back on the login screen a moment later.
            #[cfg(target_os = "linux")]
            ipc::Data::CmWindowClosed => {
                self.chat_unanswered = false; // seen
                self.file_transferred = false; //seen
                self.on_close("connection manager window closed", true).await;
                return false;
            }
            ipc::Data::CmErr(e) => {
                if e != "expected" {
                    // cm closed before connection
                    self.on_close(&format!("connection manager error: {}", e), false).await;
                    return false;
                }
            }
            ipc::Data::ChatMessage{text} => {
                let mut misc = Misc::new();
                misc.set_chat_message(ChatMessage {
                    text,
                    ..Default::default()
                });
                let mut msg_out = Message::new();
                msg_out.set_misc(misc);
                self.send(msg_out).await;
                self.chat_unanswered = false;
            }
            ipc::Data::SwitchPermission{name, enabled} => {
                self.handle_switch_permission(name, enabled).await;
            }
            ipc::Data::RawMessage(bytes) => {
                allow_err!(self.stream.send_raw(bytes).await);
            }
            #[cfg(target_os = "windows")]
            ipc::Data::ClipboardFile(clip) => {
                if !self.is_remote() {
                    return true;
                }
                match clip {
                    clipboard::ClipboardFile::Files { files } => {
                        let files = files.into_iter().map(|(f, s)| {
                            (f, s as i64)
                        }).collect::<Vec<_>>();
                        self.post_file_audit(
                            FileAuditType::RemoteSend,
                            "",
                            files,
                            json!({}),
                        );
                    }
                    _ => {
                        allow_err!(self.stream.send(&clip_2_msg(clip)).await);
                    }
                }
            }
            ipc::Data::PrivacyModeState((_, state, impl_key)) => {
                let msg_out = match state {
                    privacy_mode::PrivacyModeState::OffSucceeded => {
                        crate::common::make_privacy_mode_msg(
                            back_notification::PrivacyModeState::PrvOffSucceeded,
                            impl_key,
                        )
                    }
                    privacy_mode::PrivacyModeState::OffByPeer => {
                        crate::common::make_privacy_mode_msg(
                            back_notification::PrivacyModeState::PrvOffByPeer,
                            impl_key,
                        )
                    }
                    privacy_mode::PrivacyModeState::OffUnknown => {
                         crate::common::make_privacy_mode_msg(
                            back_notification::PrivacyModeState::PrvOffUnknown,
                            impl_key,
                        )
                    }
                };
                self.send(msg_out).await;
            }
            #[cfg(windows)]
            ipc::Data::DataPortableService(ipc::DataPortableService::RequestStart) => {
                if let Err(e) = portable_client::start_portable_service(portable_client::StartPara::Direct) {
                    log::error!("Failed to start portable service from cm: {:?}", e);
                }
            }
            #[cfg(feature = "flutter")]
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            ipc::Data::SwitchSidesBack => {
                let mut misc = Misc::new();
                misc.set_switch_back(SwitchBack::default());
                let mut msg = Message::new();
                msg.set_misc(misc);
                self.send(msg).await;
            }
            ipc::Data::VoiceCallResponse(accepted) => {
                self.handle_voice_call(accepted).await;
            }
            ipc::Data::CloseVoiceCall(_reason) => {
                log::debug!("Close the voice call from the ipc.");
                self.close_voice_call().await;
                // Notify the peer that we closed the voice call.
                let msg = new_voice_call_request(false);
                self.send(msg).await;
            }
            ipc::Data::ReadJobInitResult { id, file_num, include_hidden, conn_id, result } => {
                if conn_id == self.inner.id() {
                    self.handle_read_job_init_result(id, file_num, include_hidden, result).await;
                }
            }
            ipc::Data::FileBlockFromCM { id, file_num, data, compressed, conn_id } => {
                if conn_id == self.inner.id() {
                    self.handle_file_block_from_cm(id, file_num, data, compressed).await;
                }
            }
            ipc::Data::FileReadDone { id, file_num, conn_id } => {
                if conn_id == self.inner.id() {
                    self.handle_file_read_done(id, file_num).await;
                }
            }
            ipc::Data::FileReadError { id, file_num, err, conn_id } => {
                if conn_id == self.inner.id() {
                    self.handle_file_read_error(id, file_num, err).await;
                }
            }
            ipc::Data::FileDigestFromCM { id, file_num, last_modified, file_size, is_resume, conn_id } => {
                if conn_id == self.inner.id() {
                    self.handle_file_digest_from_cm(id, file_num, last_modified, file_size, is_resume).await;
                }
            }
            ipc::Data::AllFilesResult { id, conn_id, path, result } => {
                if conn_id == self.inner.id() {
                    self.handle_all_files_result(id, path, result).await;
                }
            }
            _ => {}
        }
        true
    }
}
