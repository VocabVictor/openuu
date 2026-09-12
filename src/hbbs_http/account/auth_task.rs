use super::*;

impl OidcSession {
    pub(super) fn auth_task(
        api_server: String,
        op: String,
        id: String,
        uuid: String,
        remember_me: bool,
        auth_attempt: u64,
    ) {
        let auth_request_res = Self::auth(&api_server, &op, &id, &uuid);
        log::info!("Request oidc auth result: {:?}", &auth_request_res);
        if !Self::auth_attempt_is_current(auth_attempt) {
            return;
        }
        let code_url = match auth_request_res {
            Ok(HbbHttpResponse::<_>::Data(code_url)) => code_url,
            Ok(HbbHttpResponse::<_>::Error(err)) => {
                Self::set_state_if_current(auth_attempt, REQUESTING_ACCOUNT_AUTH, err);
                return;
            }
            Ok(_) => {
                Self::set_state_if_current(
                    auth_attempt,
                    REQUESTING_ACCOUNT_AUTH,
                    "Invalid auth response".to_owned(),
                );
                return;
            }
            Err(err) => {
                Self::set_state_if_current(auth_attempt, REQUESTING_ACCOUNT_AUTH, err.to_string());
                return;
            }
        };

        {
            let mut session = OIDC_SESSION.write().unwrap();
            if !session.is_current_auth_attempt(auth_attempt) {
                return;
            }
            session.set_state(WAITING_ACCOUNT_AUTH, "".to_owned());
            session.code_url = Some(code_url.clone());
        }

        let begin = Instant::now();
        let query_timeout = OIDC_SESSION.read().unwrap().query_timeout;
        while Self::auth_attempt_is_current(auth_attempt) && begin.elapsed() < query_timeout {
            let query_result = Self::query(&api_server, &code_url.code, &id, &uuid);
            if !Self::auth_attempt_is_current(auth_attempt) {
                return;
            }
            match query_result {
                Ok(HbbHttpResponse::<_>::Data(auth_body)) => {
                    let mut session = OIDC_SESSION.write().unwrap();
                    if !session.is_current_auth_attempt(auth_attempt) {
                        return;
                    }
                    if auth_body.r#type == "access_token" {
                        if remember_me {
                            LocalConfig::set_option(
                                "access_token".to_owned(),
                                auth_body.access_token.clone(),
                            );
                            LocalConfig::set_option(
                                "user_info".to_owned(),
                                serde_json::json!({
                                    "name": auth_body.user.name,
                                    "display_name": auth_body.user.display_name,
                                    "avatar": auth_body.user.avatar,
                                    "status": auth_body.user.status
                                })
                                .to_string(),
                            );
                        }
                    }
                    session.set_state(LOGIN_ACCOUNT_AUTH, "".to_owned());
                    session.auth_body = Some(auth_body);
                    return;
                }
                Ok(HbbHttpResponse::<_>::Error(err)) => {
                    if err.contains("No authed oidc is found") {
                        // ignore, keep querying
                    } else {
                        Self::set_state_if_current(auth_attempt, WAITING_ACCOUNT_AUTH, err);
                        return;
                    }
                }
                Ok(_) => {
                    // ignore
                }
                Err(err) => {
                    log::trace!("Failed query oidc {}", err);
                    // ignore
                }
            }
            Self::sleep(QUERY_INTERVAL_SECS);
        }

        if begin.elapsed() >= query_timeout && Self::auth_attempt_is_current(auth_attempt) {
            Self::set_state_if_current(auth_attempt, WAITING_ACCOUNT_AUTH, "timeout".to_owned());
        }
    }

    pub(super) fn set_state(&mut self, state_msg: &'static str, failed_msg: String) {
        self.state_msg = state_msg;
        self.failed_msg = failed_msg;
    }

    pub(super) fn wait_stop_querying() {
        let wait_secs = 0.3;
        while OIDC_SESSION.read().unwrap().running {
            Self::sleep(wait_secs);
        }
    }

    pub fn account_auth(
        api_server: String,
        op: String,
        id: String,
        uuid: String,
        remember_me: bool,
    ) {
        let auth_attempt = OIDC_SESSION.write().unwrap().start_auth_attempt();
        Self::wait_stop_querying();
        {
            let mut session = OIDC_SESSION.write().unwrap();
            if !session.is_current_auth_attempt(auth_attempt) {
                return;
            }
            session.before_task();
        }
        std::thread::spawn(move || {
            Self::auth_task(api_server, op, id, uuid, remember_me, auth_attempt);
            OIDC_SESSION.write().unwrap().after_task();
        });
    }

    pub(super) fn get_result_(&self) -> AuthResult {
        AuthResult {
            state_msg: self.state_msg.to_string(),
            failed_msg: self.failed_msg.clone(),
            url: self.code_url.as_ref().map(|x| x.url.to_string()),
            auth_body: self.auth_body.clone(),
        }
    }

    pub fn auth_cancel() {
        OIDC_SESSION.write().unwrap().cancel_auth_attempt();
    }

    pub fn get_result() -> AuthResult {
        OIDC_SESSION.read().unwrap().get_result_()
    }
}
