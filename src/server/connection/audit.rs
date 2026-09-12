use super::*;

impl Connection {
    pub(super) fn get_api_server(&mut self) {
        self.server_audit_conn = crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            "conn".to_owned(),
        );
        self.server_audit_file = crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            "file".to_owned(),
        );
    }

    pub(super) fn conn_audit_ref(&self) -> Option<&str> {
        let audit_ref = self
            .controlled_context
            .as_ref()
            .map(|c| c.conn_audit_ref.as_str())?;
        if audit_ref.is_empty() {
            None
        } else {
            Some(audit_ref)
        }
    }

    pub(super) fn post_conn_audit(&self, v: Value) {
        if self.server_audit_conn.is_empty() {
            return;
        }
        let url = self.server_audit_conn.clone();
        let mut v = v;
        v["id"] = json!(Config::get_id());
        v["uuid"] = json!(crate::encode64(hbb_common::get_uuid()));
        v["conn_id"] = json!(self.inner.id);
        v["session_id"] = json!(self.lr.session_id);
        // Unique per record; the api server dedups retried posts by it.
        v["nonce"] = json!(uuid::Uuid::new_v4().to_string());
        allow_err!(self.tx_post_seq.send((url, v)));
    }

    pub(super) fn get_files_for_audit(_job_type: fs::JobType, mut files: Vec<FileEntry>) -> Vec<(String, i64)> {
        files
            .drain(..)
            .map(|f| {
                (
                    f.name,
                    f.size as _,
                )
            })
            .collect()
    }

    pub(super) fn post_file_audit(
        &self,
        r#type: FileAuditType,
        path: &str,
        files: Vec<(String, i64)>,
        info: Value,
    ) {
        if self.server_audit_file.is_empty() {
            return;
        }
        let url = self.server_audit_file.clone();
        let file_num = files.len();
        let mut files = files;
        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.truncate(10);
        let is_file = files.len() == 1 && files[0].0.is_empty();
        let mut info = info;
        info["ip"] = json!(self.ip.clone());
        info["name"] = json!(self.lr.my_name.clone());
        info["num"] = json!(file_num);
        info["files"] = json!(files);
        let v = json!({
            "id":json!(Config::get_id()),
            "uuid":json!(crate::encode64(hbb_common::get_uuid())),
            "peer_id":json!(self.lr.my_id),
            "conn_id":json!(self.inner.id()),
            "type": r#type as i8,
            "path":path,
            "is_file":is_file,
            "info":json!(info).to_string(),
            "nonce": uuid::Uuid::new_v4().to_string(),
        });
        tokio::spawn(async move {
            allow_err!(Self::post_audit_async(url, v).await);
        });
    }

    pub(super) fn post_alarm_audit(&self, typ: AlarmAuditType, info: Value) {
        let url = crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            "alarm".to_owned(),
        );
        if url.is_empty() {
            return;
        }
        let mut v = Value::default();
        v["id"] = json!(Config::get_id());
        v["uuid"] = json!(crate::encode64(hbb_common::get_uuid()));
        v["typ"] = json!(typ as i8);
        v["info"] = serde_json::Value::String(info.to_string());
        v["conn_id"] = json!(self.inner.id());
        v["nonce"] = json!(uuid::Uuid::new_v4().to_string());
        if typ == AlarmAuditType::IpWhitelist || typ == AlarmAuditType::IdWhitelist {
            if let Some(audit_ref) = self.conn_audit_ref() {
                v["conn_audit_ref"] = json!(audit_ref);
            }
        }
        tokio::spawn(async move {
            allow_err!(Self::post_audit_async(url, v).await);
        });
    }

    pub(super) fn post_session_scope_violation_alarm(&self, message: &'static str) {
        let conn_type = self
            .authed_conn_type()
            .map(AuthConnType::as_str)
            .unwrap_or("unknown");
        self.post_alarm_audit(
            AlarmAuditType::SessionScopeViolation,
            json!({
                "id": self.lr.my_id.clone(),
                "name": self.lr.my_name.clone(),
                "ip": &self.ip,
                "conn_type": conn_type,
                "message": message,
            }),
        );
    }

    pub(super) async fn post_audit_async(url: String, v: Value) -> ResultType<String> {
        // Audit records are compliance evidence; retry transport errors and
        // 5xx (e.g. a reverse proxy answering while the api server restarts)
        // so transient failures don't silently drop them. A 4xx is a
        // deterministic rejection and fails immediately.
        //
        // The delays, not the attempt count, are what cover the case this exists
        // for: a proxy answering 502 during a restart fails fast, so without them
        // every attempt lands within a few seconds and none outlives the restart.
        //
        // The window is bounded on the other side: the api server only remembers a
        // record's nonce for five minutes, so a retry arriving after that expired
        // would be stored a second time. Counting attempts cannot bound it - one
        // attempt is already up to 84s (post_request_ retries the TLS handshake up
        // to four times at 12s each, then the TCP-proxy fallback adds 36s), and a
        // suspend between attempts stretches the wall clock without limit. So stop
        // by elapsed time instead, early enough that the last attempt still lands
        // inside the server's window.
        const RETRY_DEADLINE: Duration = Duration::from_secs(120);
        // One delay per retry, so the attempt count follows from the table and the
        // two cannot drift apart.
        const RETRY_BACKOFF_SECS: [u64; 2] = [10, 30];
        const ATTEMPTS: usize = RETRY_BACKOFF_SECS.len() + 1;
        let body = v.to_string();
        let started = Instant::now();
        let mut attempt = 0usize;
        loop {
            attempt += 1;
            let (retryable, err) =
                match crate::post_request_with_status(url.clone(), body.clone(), "").await {
                    Ok((status, text)) => {
                        if (200..300).contains(&status) {
                            // Success is an empty body. hbbs reports handler
                            // failures (e.g. a db write error) as 200 with an
                            // {"error": ...} body - retryable: the server
                            // releases the record's nonce when its write fails,
                            // so trying again is what stores the record. Any
                            // other nonempty body did not come from the audit
                            // handler (a proxy interposing a 2xx maintenance
                            // page, a malformed error) and must not be mistaken
                            // for storage, so it is retried rather than dropped.
                            if text.trim().is_empty() {
                                return Ok(text);
                            }
                            let server_err = serde_json::from_str::<Value>(&text)
                                .ok()
                                .and_then(|v| v.get("error")?.as_str().map(|s| s.to_owned()))
                                .filter(|e| !e.is_empty());
                            let (label, detail) = match &server_err {
                                Some(e) => ("server error", e.as_str()),
                                None => ("unexpected response body", text.as_str()),
                            };
                            let brief: String = detail.chars().take(128).collect();
                            (true, format!("{}: {}", label, brief))
                        } else {
                            let brief: String = text.chars().take(128).collect();
                            // 408 and 429 are the transient 4xx: the request timed
                            // out upstream, or a proxy is shedding load. Every other
                            // 4xx is a deterministic rejection and retrying it would
                            // only delay the log line.
                            let transient = status >= 500 || status == 408 || status == 429;
                            (transient, format!("status {}: {}", status, brief))
                        }
                    }
                    Err(e) => (true, e.to_string()),
                };
            let elapsed = started.elapsed();
            if !retryable || attempt >= ATTEMPTS || elapsed >= RETRY_DEADLINE {
                log::error!(
                    "Audit post dropped (attempt {}/{}, {:?} elapsed): {}",
                    attempt,
                    ATTEMPTS,
                    elapsed,
                    err
                );
                bail!("{}", err);
            }
            log::warn!(
                "Audit post failed (attempt {}/{}): {}",
                attempt,
                ATTEMPTS,
                err
            );
            // In range by construction: the guard above returns at ATTEMPTS.
            time::sleep(Duration::from_secs(RETRY_BACKOFF_SECS[attempt - 1])).await;
            // Re-checked after the delay so no attempt starts past the deadline;
            // the check above alone would let one begin up to a backoff later.
            if started.elapsed() >= RETRY_DEADLINE {
                log::error!(
                    "Audit post dropped (attempt {}/{}, deadline passed during backoff): {}",
                    attempt,
                    ATTEMPTS,
                    err
                );
                bail!("{}", err);
            }
        }
    }

    pub(super) fn set_conn_audit_primary_auth(&mut self, method: ConnAuditPrimaryAuth) {
        self.conn_audit_primary_auth = method;
    }

    pub(super) fn set_conn_audit_two_factor(&mut self, two_factor: ConnAuditTwoFactor) {
        self.conn_audit_two_factor = two_factor;
    }

    pub(super) fn normalize_conn_audit_auth_fields(&mut self) {
        if matches!(
            self.conn_audit_primary_auth,
            ConnAuditPrimaryAuth::Click | ConnAuditPrimaryAuth::SwitchSides
        ) {
            self.conn_audit_two_factor = ConnAuditTwoFactor::None;
        }
    }
}
