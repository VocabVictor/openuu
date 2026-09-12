use super::*;

impl Connection {
    // Try to parse connection IP as IPv6 address, returning /64, /56, and /48 prefixes.
    // Parsing an IPv4 address just returns None.
    // note: we specifically don't use hbb_common::is_ipv6_str to avoid divergence issues
    // between its regex and the system std::net::Ipv6Addr implementation.
    pub(super) fn get_ipv6_prefixes(&self) -> Option<(String, String, String)> {
        fn mask_u128(addr: u128, prefix: u8) -> u128 {
            let mask = if prefix == 0 || prefix > 128 {
                0
            } else {
                (!0u128) << (128 - prefix)
            };
            addr & mask
        }
        // eliminate zone-ids like "fe80::1%eth0"
        let ip_only = self.ip.split('%').next().unwrap_or(&self.ip).trim();
        let ip = Ipv6Addr::from_str(ip_only).ok()?;

        let as_u128 = u128::from_be_bytes(ip.octets());

        let p64 = Ipv6Addr::from(mask_u128(as_u128, 64).to_be_bytes()).to_string() + "/64";
        let p56 = Ipv6Addr::from(mask_u128(as_u128, 56).to_be_bytes()).to_string() + "/56";
        let p48 = Ipv6Addr::from(mask_u128(as_u128, 48).to_be_bytes()).to_string() + "/48";

        Some((p64, p56, p48))
    }

    pub(super) fn bump_failure_entry(mut cur: (i32, i32, i32), time: i32) -> (i32, i32, i32) {
        if cur.0 == time {
            cur.1 += 1;
            cur.2 += 1;
        } else {
            cur.0 = time;
            cur.1 = 1;
            cur.2 += 1;
        }
        cur
    }

    pub(super) fn update_failure(&self, failure: ((i32, i32, i32), i32), remove: bool, i: usize) {
        self.update_failure_with_scope(failure, remove, i, FailureScope::Default);
    }

    pub(super) fn update_failure_with_scope(
        &self,
        (failure, time): ((i32, i32, i32), i32),
        remove: bool,
        i: usize,
        scope: FailureScope,
    ) {
        let os_credential_scope = matches!(scope, FailureScope::TerminalOsLogin);
        if os_credential_scope {
            if !remove {
                record_os_credential_failure(scope);
            }
            return;
        }

        let map_mutex = &LOGIN_FAILURES[i];
        if remove {
            if failure.0 != 0 {
                if let Some((p64, p56, p48)) = self.get_ipv6_prefixes() {
                    let mut m = map_mutex.lock().unwrap();
                    m.remove(&p64);
                    m.remove(&p56);
                    m.remove(&p48);
                    m.remove(&self.ip);
                } else {
                    map_mutex.lock().unwrap().remove(&self.ip);
                }
            }
            return;
        }
        // Bump the prefixes, fetching existing values
        if let Some((p64, p56, p48)) = self.get_ipv6_prefixes() {
            let mut m = map_mutex.lock().unwrap();
            for key in [p64, p56, p48] {
                let cur = m.get(&key).copied().unwrap_or((0, 0, 0));
                m.insert(key, Self::bump_failure_entry(cur, time));
            }
            let current_ip = m.get(&self.ip).copied().unwrap_or((0, 0, 0));
            m.insert(self.ip.clone(), Self::bump_failure_entry(current_ip, time));
        } else {
            // Re-read the full IP bucket in case another failed attempt updated it.
            let mut m = map_mutex.lock().unwrap();
            let current_ip = m.get(&self.ip).copied().unwrap_or((0, 0, 0));
            m.insert(self.ip.clone(), Self::bump_failure_entry(current_ip, time));
        }
    }

    pub(super) async fn check_failure_ipv6_prefix(
        &mut self,
        i: usize,
        time: i32,
        prefix: &str,
        prefix_num: i8,
        thresh: i32,
    ) -> Option<(((i32, i32, i32), i32), bool)> {
        let failure_prefix = LOGIN_FAILURES[i]
            .lock()
            .unwrap()
            .get(prefix)
            .copied()
            .unwrap_or((0, 0, 0));

        if failure_prefix.2 > thresh {
            self.send_login_error(format!(
                "Too many wrong attempts for IPv6 prefix /{}",
                prefix_num
            ))
            .await;
            self.post_alarm_audit(
                AlarmAuditType::ExceedIPv6PrefixAttempts,
                json!({
                            "ip": self.ip,
                            "id": self.lr.my_id.clone(),
                            "name": self.lr.my_name.clone(),
                }),
            );
            Some(((failure_prefix, time), false))
        } else {
            None
        }
    }

    pub(super) async fn check_failure(&mut self, i: usize) -> (((i32, i32, i32), i32), bool) {
        self.check_failure_with_scope(i, FailureScope::Default)
            .await
    }

    pub(super) async fn check_failure_with_scope(
        &mut self,
        i: usize,
        scope: FailureScope,
    ) -> (((i32, i32, i32), i32), bool) {
        let time = (get_time() / 60_000) as i32;

        if matches!(scope, FailureScope::TerminalOsLogin) {
            let decision = evaluate_os_credential_policy(scope, get_time());
            let res = if decision.allowed {
                true
            } else {
                log::warn!(
                    "OS credential login blocked by policy: ip={} conn_id={} i={} msg='{}'",
                    self.ip,
                    self.inner.id(),
                    i,
                    decision.login_error.as_deref().unwrap_or("")
                );
                if let Some(login_error) = decision.login_error {
                    // Rare branch and currently temporary response copy; translation can be added later if needed.
                    self.send_login_error(login_error).await;
                }
                if let Some(audit) = decision.audit {
                    // For OS blocked/backoff events, we currently emit one alarm report per blocked attempt.
                    // TODO: Add unified cumulative/aggregation fields across alarm producers.
                    self.post_alarm_audit(
                        audit,
                        json!({
                                    "ip": self.ip,
                                    "id": self.lr.my_id.clone(),
                                    "name": self.lr.my_name.clone(),
                        }),
                    );
                }
                false
            };
            return (((0, 0, 0), time), res);
        }

        // IPv6 addresses are cheap to make so we check prefix/netblock as well
        if let Some((p64, p56, p48)) = self.get_ipv6_prefixes() {
            if let Some(res) = self.check_failure_ipv6_prefix(i, time, &p64, 64, 60).await {
                return res;
            }
            if let Some(res) = self.check_failure_ipv6_prefix(i, time, &p56, 56, 80).await {
                return res;
            }
            if let Some(res) = self.check_failure_ipv6_prefix(i, time, &p48, 48, 100).await {
                return res;
            }
        }

        // checks IPv6 and IPv4 direct addresses
        let failure = LOGIN_FAILURES[i]
            .lock()
            .unwrap()
            .get(&self.ip)
            .copied()
            .unwrap_or((0, 0, 0));

        let res = if failure.2 > 30 {
            self.send_login_error("Too many wrong attempts").await;
            self.post_alarm_audit(
                AlarmAuditType::ExceedThirtyAttempts,
                json!({
                            "ip": self.ip,
                            "id": self.lr.my_id.clone(),
                            "name": self.lr.my_name.clone(),
                }),
            );
            false
        } else if time == failure.0 && failure.1 > 6 {
            self.send_login_error("Please try 1 minute later").await;
            self.post_alarm_audit(
                AlarmAuditType::SixAttemptsWithinOneMinute,
                json!({
                            "ip": self.ip,
                            "id": self.lr.my_id.clone(),
                            "name": self.lr.my_name.clone(),
                }),
            );
            false
        } else {
            true
        };
        ((failure, time), res)
    }
}
