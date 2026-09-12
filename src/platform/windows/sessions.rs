//! Pin incoming connections to one Windows user's session.
//!
//! The service normally follows whatever `get_current_session` reports, so a
//! new RDP logon steals the served session. When a user name is persisted in
//! `OPTION_PINNED_WINDOWS_SESSION` the service stays on that user's session
//! instead. The value is a user name rather than a session id because RDP
//! session ids change on every logon.
//!
//! The service process never sees option changes made through the `--server`
//! process, so the option is read from the config file directly.

use super::{get_available_sessions, get_session_username, is_share_rdp};
use base::{config::keys::OPTION_PINNED_WINDOWS_SESSION, message_proto::WindowsSession};
use hbb_common::{
    config::{Config, Config2},
    log,
};
use std::time::{Duration, Instant};

const CHECK_INTERVAL: Duration = Duration::from_secs(5);

pub fn get_pinnable_sessions_json() -> String {
    let sessions: Vec<_> = get_available_sessions(true)
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "user": get_session_username(s.sid),
                "name": s.name,
            })
        })
        .filter(|s| s["user"].as_str().map(|u| !u.is_empty()).unwrap_or(false))
        .collect();
    serde_json::to_string(&sessions).unwrap_or_else(|_| "[]".to_owned())
}

/// Persist the user of the session a controller picked in the session dialog,
/// so the service keeps serving it after the controller disconnects.
pub fn pin_session_from_selection(sid: u32, sessions: &[WindowsSession]) {
    if !super::is_installed() || !sessions.iter().any(|e| e.sid == sid) {
        return;
    }
    let user = get_session_username(sid);
    if user.is_empty() {
        log::info!("session {} has no logged on user, releasing the pinned session", sid);
    }
    Config::set_option(OPTION_PINNED_WINDOWS_SESSION.to_owned(), user);
}

pub(super) fn resolve_pinned_session(pinned_user: &str, sessions: &[(u32, String)]) -> Option<u32> {
    let pinned = pinned_user.trim();
    if pinned.is_empty() {
        return None;
    }
    let pinned = pinned.to_lowercase();
    sessions
        .iter()
        .find(|(_, user)| user.to_lowercase() == pinned)
        .map(|(sid, _)| *sid)
}

/// Extract the pinned user from the raw config file text without loading the
/// whole config into the process-wide cache.
pub(super) fn parse_pinned_session_user(config_toml: &str) -> String {
    for line in config_toml.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().trim_matches('"') != OPTION_PINNED_WINDOWS_SESSION {
            continue;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(value);
        return value.replace("\\\"", "\"").replace("\\\\", "\\");
    }
    String::new()
}

fn pinned_session_user_from_file() -> String {
    match std::fs::read_to_string(Config2::file()) {
        Ok(text) => parse_pinned_session_user(&text),
        Err(err) => {
            log::debug!("Failed to read config for the pinned session: {}", err);
            String::new()
        }
    }
}

fn sessions_with_users() -> Vec<(u32, String)> {
    get_available_sessions(false)
        .into_iter()
        .map(|s| (s.sid, get_session_username(s.sid)))
        .collect()
}

/// Rate-limited lookup of the pinned session for the service main loop.
pub struct PinnedSession {
    last_check: Option<Instant>,
    warned_missing: bool,
}

impl PinnedSession {
    pub fn new() -> Self {
        Self {
            last_check: None,
            warned_missing: false,
        }
    }

    /// `None` when no lookup ran this tick. `Some(None)` when nothing is
    /// pinned, RDP sharing is off, or the pinned user is not logged on.
    pub fn resolve(&mut self) -> Option<Option<u32>> {
        if self
            .last_check
            .map(|t| t.elapsed() < CHECK_INTERVAL)
            .unwrap_or(false)
        {
            return None;
        }
        self.last_check = Some(Instant::now());
        let pinned_user = pinned_session_user_from_file();
        if pinned_user.trim().is_empty() {
            self.warned_missing = false;
            return Some(None);
        }
        if !is_share_rdp() {
            if !self.warned_missing {
                log::warn!(
                    "pinned session user {:?} ignored because RDP session sharing is disabled",
                    pinned_user
                );
                self.warned_missing = true;
            }
            return Some(None);
        }
        let sid = resolve_pinned_session(&pinned_user, &sessions_with_users());
        match sid {
            Some(_) => self.warned_missing = false,
            None if !self.warned_missing => {
                log::warn!(
                    "pinned session user {:?} is not logged on, following the active session",
                    pinned_user
                );
                self.warned_missing = true;
            }
            None => {}
        }
        Some(sid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessions() -> Vec<(u32, String)> {
        vec![
            (1, "Administrator".to_owned()),
            (3, "alice".to_owned()),
            (4, "".to_owned()),
        ]
    }

    #[test]
    fn resolves_case_insensitively_and_prefers_first_match() {
        assert_eq!(resolve_pinned_session("administrator", &sessions()), Some(1));
        assert_eq!(resolve_pinned_session(" ALICE ", &sessions()), Some(3));
    }

    #[test]
    fn falls_back_when_unset_or_not_logged_on() {
        assert_eq!(resolve_pinned_session("", &sessions()), None);
        assert_eq!(resolve_pinned_session("   ", &sessions()), None);
        assert_eq!(resolve_pinned_session("nobody", &sessions()), None);
        assert_eq!(resolve_pinned_session("Administrator", &[]), None);
    }

    #[test]
    fn parses_option_from_config_text() {
        let text = "[options]\nkey = \"x\"\npinned-windows-session = \"Admin\\\\istrator\"\n";
        assert_eq!(parse_pinned_session_user(text), "Admin\\istrator");
        let quoted = "\"pinned-windows-session\" = \"alice\"";
        assert_eq!(parse_pinned_session_user(quoted), "alice");
        assert_eq!(parse_pinned_session_user("[options]\nother = \"1\"\n"), "");
        assert_eq!(parse_pinned_session_user(""), "");
    }
}
