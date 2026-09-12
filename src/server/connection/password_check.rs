use super::*;

impl Connection {
    pub(super) fn verify_h1(&self, h1: &[u8]) -> bool {
        let mut hasher2 = Sha256::new();
        hasher2.update(h1);
        hasher2.update(self.hash.challenge.as_bytes());
        // A normal `==` on slices may short-circuit on the first mismatch, which can leak how many leading
        // bytes matched via timing. In typical remote scenarios this is difficult to exploit due to network
        // jitter, changing challenges, and login attempt throttling, but a constant-time comparison here is
        // low-cost defensive programming.
        constant_time_eq(&hasher2.finalize()[..], &self.lr.password[..])
    }

    pub(super) fn validate_password_plain(&self, password: &str) -> bool {
        if password.is_empty() {
            return false;
        }

        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(self.hash.salt.as_bytes());
        let h1_plain = hasher.finalize();
        self.verify_h1(&h1_plain[..])
    }

    pub(super) fn validate_password_storage(&self, storage: &str) -> bool {
        if storage.is_empty() {
            return false;
        }

        // Use strict decode success to detect hashed storage.
        // If decode fails, treat as legacy plaintext storage for compatibility.
        if let Some(h1) = decode_permanent_password_h1_from_storage(storage) {
            return self.verify_h1(&h1[..]);
        }

        // Legacy plaintext storage path.
        self.validate_password_plain(storage)
    }

    pub(super) fn validate_preset_password_storage(&self, storage: &str, salt: &str) -> bool {
        if salt.is_empty() {
            return self.validate_password_plain(storage);
        }
        let Some(h1) = decode_preset_password_h1_from_storage(storage) else {
            return false;
        };
        self.verify_h1(&h1[..])
    }

    // This is coarse brute-force protection for the current temporary password value.
    // We only care whether the active temporary password itself was presented correctly,
    // not whether later authorization steps succeed. A successful temporary-password
    // match clears this state immediately, and the counter also resets whenever the
    // temporary password changes or is rotated.
    pub(super) fn check_update_temporary_password(&self, temporary_password_success: bool) {
        const MAX_CONSECUTIVE_FAILURES: i32 = 10;
        #[derive(Default)]
        struct State {
            password: String,
            failures: i32,
        }
        lazy_static::lazy_static! {
            static ref TEMPORARY_PASSWORD_FAILURES: Mutex<State> =
                Mutex::new(State::default());
        }

        if !password::temporary_enabled() {
            return;
        }

        let mut state = TEMPORARY_PASSWORD_FAILURES.lock().unwrap();
        let current_password = password::temporary_password();
        if current_password.is_empty() {
            return;
        }
        if state.password != current_password {
            state.password = current_password;
            state.failures = 0;
        }

        if temporary_password_success {
            state.failures = 0;
            return;
        }
        state.failures += 1;

        if state.failures < MAX_CONSECUTIVE_FAILURES {
            return;
        }

        password::update_temporary_password();
        let new_password = password::temporary_password();
        log::warn!(
            "Temporary password rotated after too many consecutive wrong attempts: failures={}, ip={}",
            state.failures,
            self.ip,
        );
        state.password = new_password;
        state.failures = 0;
    }

    pub(super) fn validate_password(&mut self, allow_permanent_password: bool) -> bool {
        if password::temporary_enabled() {
            let password = password::temporary_password();
            if self.validate_password_plain(&password) {
                self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::TemporaryPassword);
                raii::AuthedConnID::update_or_insert_session(
                    self.session_key(),
                    Some(password),
                    Some(false),
                );
                self.check_update_temporary_password(true);
                return true;
            }
        }
        if password::permanent_enabled() || allow_permanent_password {
            let print_fallback = || {
                if allow_permanent_password && !password::permanent_enabled() {
                    log::info!("Permanent password accepted via logon-screen fallback");
                }
            };
            // Strictly check storage usability before auth so malformed encrypted/hash storage
            // cannot fall back to being accepted as legacy plaintext.
            let (local_storage, local_salt) =
                Config::get_local_permanent_password_storage_and_salt();
            if !local_storage.is_empty() {
                if local_permanent_password_storage_is_usable_for_auth(&local_storage, &local_salt)
                    && self.validate_password_storage(&local_storage)
                {
                    self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::PermanentPassword);
                    print_fallback();
                    return true;
                }
            } else {
                let (hard, salt) = Config::get_preset_password_storage_and_salt();
                if preset_permanent_password_storage_is_usable_for_auth(&hard, &salt)
                    && self.validate_preset_password_storage(&hard, &salt)
                {
                    self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::PermanentPassword);
                    print_fallback();
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn is_recent_session(&mut self, tfa: bool) -> bool {
        SESSIONS
            .lock()
            .unwrap()
            .retain(|_, s| s.last_recv_time.lock().unwrap().elapsed() < SESSION_TIMEOUT);
        let session = SESSIONS
            .lock()
            .unwrap()
            .get(&self.session_key())
            .map(|s| s.to_owned());
        // last_recv_time is a mutex variable shared with connection, can be updated lively.
        if let Some(session) = session {
            if !self.lr.password.is_empty()
                && (tfa && session.tfa
                    || !tfa && self.validate_password_plain(&session.random_password))
            {
                if tfa {
                    self.set_conn_audit_two_factor(ConnAuditTwoFactor::Totp);
                } else {
                    self.set_conn_audit_primary_auth(ConnAuditPrimaryAuth::TemporaryPassword);
                }
                log::info!("is recent session");
                return true;
            }
        }
        false
    }
}
