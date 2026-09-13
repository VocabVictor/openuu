use super::*;

impl Connection {
    /// `TestDelay`: echo the controller's probe, or record the round trip of
    /// our own.
    pub(super) fn handle_test_delay(&mut self, t: TestDelay) {
        if t.from_client {
            let mut msg_out = Message::new();
            msg_out.set_test_delay(t);
            self.inner.send(msg_out.into());
        } else {
            if let Some(tm) = self.last_test_delay {
                self.last_test_delay = None;
                let new_delay = tm.elapsed().as_millis() as u32;
                video_service::VIDEO_QOS
                    .lock()
                    .unwrap()
                    .user_network_delay(self.inner.id(), new_delay);
                self.network_delay = new_delay;
            }
        }
    }

    /// `SwitchSidesResponse`: authorize the peer that asked us to switch sides.
    /// Returns false when the connection must close.
    pub(super) async fn handle_switch_sides_response(&mut self, _s: SwitchSidesResponse) -> bool {
        #[cfg(feature = "flutter")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Some(lr) = _s.lr.clone().take() {
            SWITCH_SIDES_UUID
                .lock()
                .unwrap()
                .retain(|_, v| v.0.elapsed() < SWITCH_SIDES_UUID_TTL);
            let uuid_old = SWITCH_SIDES_UUID.lock().unwrap().remove(&lr.my_id);
            if let Ok(uuid) = uuid::Uuid::from_slice(_s.uuid.to_vec().as_ref()) {
                if let Some((_instant, uuid_old)) = uuid_old {
                    if uuid == uuid_old {
                        if lr.union.is_some() {
                            log::warn!(
                                "Rejected switch sides response for non-remote-desktop session; closing connection"
                            );
                            self.send_login_error("Connection not allowed").await;
                            return false;
                        }
                        self.reset_session_scope_for_login();
                        self.handle_login_request_without_validation(&lr).await;
                        // Switching sides authorizes without a password, so it must not bypass
                        // the whitelist, which can be a locked policy pushed by the server.
                        if !self.check_id_whitelist().await {
                            return false;
                        }
                        self.from_switch = true;
                        self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::SwitchSides);
                        if !self.send_logon_response_and_keep_alive().await {
                            return false;
                        }
                        self.try_start_cm(
                            lr.my_id.clone(),
                            lr.my_name.clone(),
                            self.authorized,
                        );
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        self.try_start_cm_ipc();
                    }
                }
            }
        }
        true
    }
}
