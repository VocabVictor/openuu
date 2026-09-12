use super::*;

impl OidcSession {
    pub(super) fn new() -> Self {
        Self {
            warmed_api_server: None,
            state_msg: REQUESTING_ACCOUNT_AUTH,
            failed_msg: "".to_owned(),
            code_url: None,
            auth_body: None,
            auth_attempt: 0,
            running: false,
            query_timeout: Duration::from_secs(QUERY_TIMEOUT_SECS),
        }
    }

    pub(super) fn ensure_client(api_server: &str) {
        let mut write_guard = OIDC_SESSION.write().unwrap();
        if write_guard.warmed_api_server.as_deref() == Some(api_server) {
            return;
        }
        // This URL is used to detect the appropriate TLS implementation for the server.
        let login_option_url = format!("{}/api/login-options", api_server);
        let _ = create_http_client_with_url(&login_option_url);
        write_guard.warmed_api_server = Some(api_server.to_owned());
    }

    pub(super) fn auth(
        api_server: &str,
        op: &str,
        id: &str,
        uuid: &str,
    ) -> ResultType<HbbHttpResponse<OidcAuthUrl>> {
        Self::ensure_client(api_server);
        let body = serde_json::json!({
            "op": op,
            "id": id,
            "uuid": uuid,
            "deviceInfo": crate::ui_interface::get_login_device_info(),
            "apiDomain": api_server,
        })
        .to_string();
        let resp = crate::post_request_sync(format!("{}/api/oidc/auth", api_server), body, "")?;
        HbbHttpResponse::parse(&resp)
    }

    pub(super) fn query(
        api_server: &str,
        code: &str,
        id: &str,
        uuid: &str,
    ) -> ResultType<HbbHttpResponse<AuthBody>> {
        let url = Url::parse_with_params(
            &format!("{}/api/oidc/auth-query", api_server),
            &[("code", code), ("id", id), ("uuid", uuid)],
        )?;
        Self::ensure_client(api_server);
        #[derive(Deserialize)]
        struct HttpResponseBody {
            body: String,
        }

        let resp =
            crate::http_request_sync(url.to_string(), "GET".to_owned(), None, "{}".to_owned())?;
        let resp = serde_json::from_str::<HttpResponseBody>(&resp)?;
        HbbHttpResponse::parse(&resp.body)
    }

    pub(super) fn reset(&mut self) {
        self.state_msg = REQUESTING_ACCOUNT_AUTH;
        self.failed_msg = "".to_owned();
        self.running = false;
        self.code_url = None;
        self.auth_body = None;
    }

    pub(super) fn before_task(&mut self) {
        self.reset();
        self.running = true;
    }

    pub(super) fn after_task(&mut self) {
        self.running = false;
    }

    pub(super) fn start_auth_attempt(&mut self) -> u64 {
        self.auth_attempt = self.auth_attempt.wrapping_add(1);
        self.auth_attempt
    }

    pub(super) fn cancel_auth_attempt(&mut self) {
        self.auth_attempt = self.auth_attempt.wrapping_add(1);
    }

    pub(super) fn is_current_auth_attempt(&self, auth_attempt: u64) -> bool {
        self.auth_attempt == auth_attempt
    }

    pub(super) fn auth_attempt_is_current(auth_attempt: u64) -> bool {
        OIDC_SESSION
            .read()
            .unwrap()
            .is_current_auth_attempt(auth_attempt)
    }

    pub(super) fn set_state_if_current(auth_attempt: u64, state_msg: &'static str, failed_msg: String) {
        let mut session = OIDC_SESSION.write().unwrap();
        if session.is_current_auth_attempt(auth_attempt) {
            session.set_state(state_msg, failed_msg);
        }
    }

    pub(super) fn sleep(secs: f32) {
        std::thread::sleep(std::time::Duration::from_secs_f32(secs));
    }
}
