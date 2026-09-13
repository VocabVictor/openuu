use super::*;

impl Connection {
    /// `LoginRequest`: scope, whitelist and privacy checks, then password or
    /// click approval. Returns false when the connection must close.
    pub(super) async fn handle_login_request(&mut self, lr: LoginRequest) -> bool {
        if !self.check_login_scope(&lr).await {
            return false;
        }
        self.awaiting_2fa = false;
        self.handle_login_request_without_validation(&lr).await;
        if self.authorized {
            return true;
        }
        self.reset_session_scope_for_login();
        if !self.check_id_whitelist().await {
            return false;
        }
        match lr.union {
            Some(login_request::Union::FileTransfer(ft)) => {
                if !Self::permission(
                    keys::OPTION_ENABLE_FILE_TRANSFER,
                    &self.control_permissions,
                ) {
                    self.send_login_error("No permission of file transfer")
                        .await;
                    sleep(1.).await;
                    return false;
                }
                self.file_transfer = Some((ft.dir, ft.show_hidden));
            }
            Some(login_request::Union::ViewCamera(_vc)) => {
                if !Self::permission(keys::OPTION_ENABLE_CAMERA, &self.control_permissions) {
                    self.send_login_error("No permission of viewing camera")
                        .await;
                    sleep(1.).await;
                    return false;
                }
                self.view_camera = true;
            }
            Some(login_request::Union::Terminal(terminal)) => {
                if !Self::permission(keys::OPTION_ENABLE_TERMINAL, &self.control_permissions) {
                    self.send_login_error("No permission of terminal").await;
                    sleep(1.).await;
                    return false;
                }
                #[cfg(target_os = "windows")]
                if !lr.os_login.username.is_empty() && !crate::platform::is_installed() {
                    self.send_login_error("Supported only in the installed version.")
                        .await;
                    sleep(1.).await;
                    return false;
                }

                self.terminal = true;
                if let Some(o) = self.options_in_login.as_ref() {
                    self.terminal_persistent =
                        o.terminal_persistent.enum_value() == Ok(BoolOption::Yes);
                }
                self.terminal_service_id = terminal.service_id;
            }
            Some(login_request::Union::PortForward(mut pf)) => {
                if !Self::permission(keys::OPTION_ENABLE_TUNNEL, &self.control_permissions) {
                    self.send_login_error("No permission of IP tunneling").await;
                    sleep(1.).await;
                    return false;
                }
                let (addr, _is_rdp) = Self::normalize_port_forward_target(&mut pf);
                self.port_forward_address = addr;
            }
            _ => {
                if !self.check_privacy_mode_on().await {
                    return false;
                }
            }
        }

        self.stream.set_send_timeout(
            if self.file_transfer.is_some()
                || self.terminal
                || matches!(self.lr.union, Some(login_request::Union::PortForward(_)))
            {
                SEND_TIMEOUT_OTHER
            } else {
                SEND_TIMEOUT_VIDEO
            },
        );

        if !crate::common::is_direct_ip_access(&lr.username) && lr.username != Config::get_id()
        {
            self.send_login_error(crate::client::LOGIN_MSG_OFFLINE)
                .await;
            return false;
        }

        #[cfg(target_os = "windows")]
        if self.terminal
            && lr.os_login.username.trim().is_empty()
            && crate::platform::is_prelogin()
        {
            self.send_login_error(
                "No active console user logged on, please connect and logon first.",
            )
            .await;
            sleep(1.).await;
            return false;
        }

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if !should_use_terminal_os_login_scope(self.terminal, &lr.os_login.username) {
            self.try_start_cm_ipc();
        }

        // https://github.com/rustdesk/rustdesk-server-pro/discussions/646
        // `is_logon` is used to check login with `OPTION_ALLOW_LOGON_SCREEN_PASSWORD` == "Y".
        // `is_logon_ui()` is a fallback for logon UI detection on Windows.
        #[cfg(target_os = "windows")]
        let is_logon = || {
            crate::platform::is_prelogin() || crate::platform::is_locked() || {
                match crate::platform::is_logon_ui() {
                    Ok(result) => result,
                    Err(e) => {
                        log::error!("Failed to detect logon UI: {:?}", e);
                        false
                    }
                }
            }
        };
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let is_logon = || crate::platform::is_prelogin() || crate::platform::is_locked();
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let is_logon = || crate::platform::is_prelogin();

        let allow_logon_screen_password =
            crate::get_builtin_option(keys::OPTION_ALLOW_LOGON_SCREEN_PASSWORD) == "Y"
                && is_logon();

        if (password::approve_mode() == ApproveMode::Click && !allow_logon_screen_password)
            || password::approve_mode() == ApproveMode::Both && !password::has_valid_password()
        {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if should_use_terminal_os_login_scope(self.terminal, &lr.os_login.username) {
                if let Some(keep_alive) = self.prepare_terminal_login_for_authorization().await
                {
                    return keep_alive;
                }
            }
            self.try_start_cm(lr.my_id, lr.my_name, false);
            if hbb_common::get_version_number(&lr.version)
                >= hbb_common::get_version_number("1.2.0")
            {
                self.send_login_error(crate::client::LOGIN_MSG_NO_PASSWORD_ACCESS)
                    .await;
            }
            return true;
        } else if self.is_recent_session(false) {
            if !self.send_logon_response_and_keep_alive().await {
                return false;
            }
            self.try_start_cm(lr.my_id.clone(), lr.my_name.clone(), self.authorized);
        } else if lr.password.is_empty() {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if should_use_terminal_os_login_scope(self.terminal, &lr.os_login.username) {
                if let Some(keep_alive) = self.prepare_terminal_login_for_authorization().await
                {
                    return keep_alive;
                }
            }
            self.try_start_cm(lr.my_id, lr.my_name, false);
        } else {
            let (failure, res) = self.check_failure(0).await;
            if !res {
                return true;
            }
            if !self.validate_password(allow_logon_screen_password) {
                self.update_failure_with_scope(failure, false, 0, FailureScope::Default);
                self.check_update_temporary_password(false);
                self.send_login_error(crate::client::LOGIN_MSG_PASSWORD_WRONG)
                    .await;
                self.try_start_cm(lr.my_id, lr.my_name, false);
            } else {
                self.update_failure_with_scope(failure, true, 0, FailureScope::Default);
                if !self.send_logon_response_and_keep_alive().await {
                    return false;
                }
                self.try_start_cm(lr.my_id, lr.my_name, self.authorized);
            }
        }
        true
    }

    /// `Auth2FA`: second factor for a login that is waiting on it.
    pub(super) async fn handle_auth_2fa(&mut self, tfa: Auth2FA) -> bool {
        // A 2FA response may arrive after click authorization has completed.
        // Ignore it unless this connection is still waiting for the response.
        if !self.awaiting_2fa {
            return true;
        }
        let (failure, res) = self.check_failure(1).await;
        if !res {
            return true;
        }
        if let Some(totp) = self.require_2fa.as_ref() {
            if let Ok(res) = totp.check_current(&tfa.code) {
                if res {
                    self.update_failure(failure, true, 1);
                    self.require_2fa.take();
                    self.set_conn_audit_two_factor(ConnAuditTwoFactor::Totp);
                    raii::AuthedConnID::set_session_2fa(self.session_key());
                    if !self.send_logon_response_and_keep_alive().await {
                        return false;
                    }
                    self.try_start_cm(
                        self.lr.my_id.to_owned(),
                        self.lr.my_name.to_owned(),
                        self.authorized,
                    );
                    if !tfa.hwid.is_empty() && Self::enable_trusted_devices() {
                        Config::add_trusted_device(TrustedDevice {
                            hwid: tfa.hwid,
                            time: hbb_common::get_time(),
                            id: self.lr.my_id.clone(),
                            name: self.lr.my_name.clone(),
                            platform: self.lr.my_platform.clone(),
                        });
                    }
                } else {
                    self.update_failure(failure, false, 1);
                    self.send_login_error(crate::client::LOGIN_MSG_2FA_WRONG)
                        .await;
                }
            }
        }
        true
    }
}
