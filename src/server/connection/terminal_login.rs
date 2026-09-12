use super::*;

impl Connection {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn fill_terminal_user_token(
        &mut self,
        _username: &str,
        _password: &str,
    ) -> Option<&'static str> {
        self.terminal_user_token = Some(TerminalUserToken::SelfUser);
        None
    }

    // Try to fill user token for terminal connection.
    // If username is empty, use the user token of the current session.
    // If username is not empty, try to logon and check if the user is an administrator.
    //    If the user is an administrator, use the user token of current process (SYSTEM).
    //    If the user is not an administrator, return an error message.
    // Note: Only local and domain users are supported, Microsoft account (online account) not supported for now.
    #[cfg(target_os = "windows")]
    pub(super) fn fill_terminal_user_token(&mut self, username: &str, password: &str) -> Option<&'static str> {
        // No need to check if the password is empty.
        if !username.is_empty() {
            return self.handle_administrator_check(username, password);
        }

        if crate::platform::is_prelogin() {
            self.terminal_user_token = None;
            return Some("No active console user logged on, please connect and logon first.");
        }

        if crate::platform::is_installed() {
            return self.handle_installed_user();
        }

        self.terminal_user_token = Some(TerminalUserToken::SelfUser);
        None
    }

    #[cfg(target_os = "windows")]
    pub(super) fn handle_administrator_check(
        &mut self,
        username: &str,
        password: &str,
    ) -> Option<&'static str> {
        let check_admin_res =
            crate::platform::get_logon_user_token(username, password).map(|token| {
                let is_token_admin = crate::platform::is_user_token_admin(token);
                unsafe {
                    hbb_common::allow_err!(CloseHandle(HANDLE(token as _)));
                };
                is_token_admin
            });
        match check_admin_res {
            Ok(Ok(b)) => {
                if b {
                    self.terminal_user_token = Some(TerminalUserToken::SelfUser);
                    None
                } else {
                    Some(TERMINAL_OS_LOGIN_FAILED_MSG)
                }
            }
            Ok(Err(e)) => {
                log::error!("Failed to check if the user is an administrator: {}", e);
                Some(TERMINAL_OS_LOGIN_FAILED_MSG)
            }
            Err(e) => {
                log::error!("Failed to get logon user token: {}", e);
                Some(TERMINAL_OS_LOGIN_FAILED_MSG)
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub(super) fn handle_installed_user(&mut self) -> Option<&'static str> {
        let session_id = crate::platform::get_current_session_id(true);
        if session_id == 0xFFFFFFFF {
            return Some("Failed to get current session id.");
        }
        let token = crate::platform::get_user_token(session_id, true);
        if !token.is_null() {
            match crate::platform::ensure_primary_token(token) {
                Ok(t) => {
                    self.terminal_user_token = Some(TerminalUserToken::CurrentLogonUser(
                        crate::terminal_service::UserToken::new(t as usize),
                    ));
                }
                Err(e) => {
                    log::error!("Failed to ensure primary token: {}", e);
                    self.terminal_user_token = Some(TerminalUserToken::CurrentLogonUser(
                        crate::terminal_service::UserToken::new(token as usize),
                    ));
                }
            }
            None
        } else {
            log::error!(
                "Failed to get user token for terminal action, {}",
                std::io::Error::last_os_error()
            );
            Some("Failed to get user token.")
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) async fn prepare_terminal_login_for_authorization(&mut self) -> Option<bool> {
        if !self.terminal || self.terminal_user_token.is_some() {
            return None;
        }

        #[derive(Copy, Clone)]
        enum TerminalAuthorizationMode {
            OsLogin {
                failure: ((i32, i32, i32), i32),
                scope: FailureScope,
            },
            SessionUser,
        }

        let normalized_username = self.lr.os_login.username.trim().to_owned();
        let auth_mode = if should_use_terminal_os_login_scope(self.terminal, &normalized_username) {
            // Check failure state
            let failure_scope = FailureScope::TerminalOsLogin;
            let (failure, res) = self.check_failure_with_scope(0, failure_scope).await;
            if !res {
                log::warn!(
                    "OS credential login blocked by failure policy: ip={} conn_id={} scope={:?}",
                    self.ip,
                    self.inner.id(),
                    failure_scope
                );
                // Terminal OS login is sensitive. Close this connection instead of keeping it
                // alive for retries on the same socket after a rate-limit block.
                return Some(false);
            }
            TerminalAuthorizationMode::OsLogin {
                failure,
                scope: failure_scope,
            }
        } else {
            TerminalAuthorizationMode::SessionUser
        };

        let is_terminal_os_login = matches!(auth_mode, TerminalAuthorizationMode::OsLogin { .. });
        let failure_scope = match auth_mode {
            TerminalAuthorizationMode::OsLogin { scope, .. } => scope,
            TerminalAuthorizationMode::SessionUser => FailureScope::Default,
        };

        let username = normalized_username;
        let password = self.lr.os_login.password.clone();
        let terminal_login_error = {
            #[cfg(target_os = "windows")]
            {
                // Concurrency gate for terminal OS login with credentials, to prevent brute-force attacks.
                let _os_login_concurrency_guard = if is_terminal_os_login {
                    let guard = try_acquire_os_credential_login_gate();
                    if guard.is_err() {
                        log::warn!(
                            "OS credential login blocked by concurrency gate: ip={} conn_id={} scope={:?}",
                            self.ip,
                            self.inner.id(),
                            failure_scope
                        );
                        self.send_login_error("Please try 1 minute later").await;
                        sleep(1.).await;
                        self.post_alarm_audit(
                            AlarmAuditType::TerminalOsLoginConcurrency,
                            json!({
                                "ip": self.ip,
                                "id": self.lr.my_id.clone(),
                                "name": self.lr.my_name.clone(),
                            }),
                        );
                        return Some(false);
                    }
                    guard.ok()
                } else {
                    None
                };
                self.fill_terminal_user_token(&username, &password)
            }
            #[cfg(not(target_os = "windows"))]
            {
                self.fill_terminal_user_token(&username, &password)
            }
        };
        if let Some(msg) = terminal_login_error {
            if let TerminalAuthorizationMode::OsLogin { failure, scope } = auth_mode {
                self.update_failure_with_scope(failure, false, 0, scope);
            }
            let auth_context = if is_terminal_os_login {
                "OS credential login verification"
            } else {
                "Terminal session-user authorization"
            };
            log::warn!(
                "{} failed: ip={} conn_id={} scope={:?} msg='{}'",
                auth_context,
                self.ip,
                self.inner.id(),
                failure_scope,
                msg
            );
            self.send_login_error(msg).await;
            sleep(1.).await;
            return Some(false);
        }
        if let TerminalAuthorizationMode::OsLogin { failure, scope } = auth_mode {
            self.update_failure_with_scope(failure, true, 0, scope);
        }

        if let Some(is_user) =
            terminal_service::is_service_specified_user(&self.terminal_service_id)
        {
            if let Some(user_token) = &self.terminal_user_token {
                let has_service_token = user_token.to_terminal_service_token().is_some();
                if is_user != has_service_token {
                    log::error!(
                        "Terminal service user mismatch: ip={} conn_id={} service_is_user={} has_service_token={}. The service ID may have been manually changed in the configuration, causing validation to fail.",
                        self.ip,
                        self.inner.id(),
                        is_user,
                        has_service_token
                    );
                    // No need to translate the following message, because it is in an abnormal case.
                    self.send_login_error("Terminal service user mismatch detected.")
                        .await;
                    sleep(1.).await;
                    return Some(false);
                }
            }
        }
        if is_terminal_os_login {
            self.try_start_cm_ipc();
        }
        None
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub(super) async fn prepare_terminal_login_for_authorization(&mut self) -> Option<bool> {
        None
    }
}
