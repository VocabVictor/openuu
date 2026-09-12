use super::*;

impl Connection {
    pub(super) async fn send_permission(&mut self, permission: Permission, enabled: bool) {
        let mut misc = Misc::new();
        misc.set_permission_info(PermissionInfo {
            permission: permission.into(),
            enabled,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(msg_out).await;
    }

    pub(super) async fn check_privacy_mode_on(&mut self) -> bool {
        if privacy_mode::is_in_privacy_mode() {
            self.send_login_error("Someone turns on privacy mode, exit")
                .await;
            false
        } else {
            true
        }
    }

    pub(super) async fn check_whitelist(&mut self, addr: &SocketAddr) -> bool {
        let whitelist: Vec<String> = Config::get_option("whitelist")
            .split(",")
            .filter(|x| !x.is_empty())
            .map(|x| x.to_owned())
            .collect();
        if !whitelist.is_empty()
            && whitelist
                .iter()
                .filter(|x| x == &"0.0.0.0")
                .next()
                .is_none()
            && whitelist
                .iter()
                .filter(|x| IpCidr::from_str(x).map_or(false, |y| y.contains(addr.ip())))
                .next()
                .is_none()
        {
            self.send_login_error("Your ip is blocked by the peer")
                .await;
            self.post_alarm_audit(
                AlarmAuditType::IpWhitelist, //"ip whitelist",
                json!({ "ip":addr.ip() }),
            );
            return false;
        }
        true
    }

    pub(super) async fn check_id_whitelist(&mut self) -> bool {
        let id_whitelist: Vec<String> = Config::get_option(keys::OPTION_ID_WHITELIST)
            .split(',')
            .map(|x| x.trim().to_owned())
            .filter(|x| !x.is_empty())
            .collect();
        if id_whitelist.is_empty() {
            return true;
        }
        // Limit before matching, or a match returning early would never touch the counter and
        // leave enumeration unthrottled. Not cleared here: `my_id` is self-reported, so that
        // would let anyone holding one allowed id reset the budget between probes.
        self.decay_id_whitelist_failures();
        let (failure, res) = self.check_failure(FAILURE_IDX_ID_WHITELIST).await;
        if !res {
            return false;
        }
        if id_whitelist_allows(&id_whitelist, &self.lr.my_id) {
            return true;
        }
        self.update_failure(failure, false, FAILURE_IDX_ID_WHITELIST);
        self.send_login_error("Your ID is blocked by the peer")
            .await;
        self.post_alarm_audit(
            AlarmAuditType::IdWhitelist,
            json!({ "id": self.lr.my_id.clone(), "ip": self.ip.clone(), "name": self.lr.my_name.clone() }),
        );
        false
    }

    // What `check_failure` consults: the source address, plus shared IPv6 prefixes.
    pub(super) fn failure_keys(&self) -> Vec<String> {
        let mut keys = vec![self.ip.clone()];
        if let Some((p64, p56, p48)) = self.get_ipv6_prefixes() {
            keys.extend([p64, p56, p48]);
        }
        keys
    }

    // Only this connection's own keys, so it stays O(1) instead of scanning the map.
    pub(super) fn decay_id_whitelist_failures(&self) {
        decay_stale_failures(
            &mut LOGIN_FAILURES[FAILURE_IDX_ID_WHITELIST].lock().unwrap(),
            &self.failure_keys(),
            (get_time() / 60_000) as i32,
            ID_WHITELIST_FAILURE_DECAY_MINUTES,
        );
    }

    // Not `update_failure(.., true, ..)`: it no-ops when the peer's own address has no entry,
    // normal on IPv6, leaving the shared prefixes that are what actually block it.
    pub(super) fn clear_id_whitelist_failures(&self) {
        clear_failures(
            &mut LOGIN_FAILURES[FAILURE_IDX_ID_WHITELIST].lock().unwrap(),
            &self.failure_keys(),
        );
    }

    pub(super) async fn on_open(&mut self, addr: SocketAddr) -> bool {
        log::debug!("#{} Connection opened from {}.", self.inner.id, addr);
        if !self.check_whitelist(&addr).await {
            return false;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if crate::is_server() && Config::get_option("allow-only-conn-window-open") == "Y" {
            if !crate::check_process("", !crate::platform::is_root()) {
                self.send_login_error("The main window is not open").await;
                return false;
            }
        }
        self.ip = addr.ip().to_string();
        let mut msg_out = Message::new();
        msg_out.set_hash(self.hash.clone());
        self.send(msg_out).await;
        self.get_api_server();
        let mut audit = json!({
            "ip": addr.ip(),
            "action": "new",
        });
        if let Some(audit_ref) = self.conn_audit_ref() {
            audit["conn_audit_ref"] = json!(audit_ref);
        }
        self.post_conn_audit(audit);
        true
    }
}
