use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(not(any(target_os = "ios")))]
use crate::{ui_interface::get_builtin_option, Connection};
use hbb_common::{
    config::{self, Config, LocalConfig},
    log,
    tokio::{self, sync::broadcast, time::Instant},
};
use base::config::keys;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const TIME_HEARTBEAT: Duration = Duration::from_secs(15);
const UPLOAD_SYSINFO_TIMEOUT: Duration = Duration::from_secs(120);
const TIME_CONN: Duration = Duration::from_secs(3);

mod heartbeat;
pub use heartbeat::*;
#[cfg(test)]
mod sysinfo_tests;
mod switch_grant;
pub use switch_grant::*;

#[cfg(not(any(target_os = "ios")))]
lazy_static::lazy_static! {
    static ref SENDER : Mutex<broadcast::Sender<Vec<i32>>> = Mutex::new(start_hbbs_sync());
    static ref PRO: Arc<Mutex<bool>> = Default::default();
}

#[cfg(not(any(target_os = "ios")))]
pub fn start() {
    let _sender = SENDER.lock().unwrap();
}

#[cfg(not(target_os = "ios"))]
pub fn signal_receiver() -> broadcast::Receiver<Vec<i32>> {
    SENDER.lock().unwrap().subscribe()
}

#[cfg(not(any(target_os = "ios")))]
fn start_hbbs_sync() -> broadcast::Sender<Vec<i32>> {
    let (tx, _rx) = broadcast::channel::<Vec<i32>>(16);
    std::thread::spawn(move || start_hbbs_sync_async());
    return tx;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyOptions {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config_options: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

struct InfoUploaded {
    uploaded: bool,
    url: String,
    last_uploaded: Option<Instant>,
    id: String,
    username: Option<String>,
}

impl InfoUploaded {
    /// The system information this heartbeat should upload, if any.
    ///
    /// Collecting it refreshes the CPU and memory counters and reads the OS version, and
    /// the heartbeat ticks every three seconds whether or not anything can come of it, so
    /// a tick that could not upload does not collect. The server asks for it again at
    /// most every [`UPLOAD_SYSINFO_TIMEOUT`], and until then nothing about the machine
    /// can make this tick upload anything.
    fn sysinfo_to_upload(&self, collect: impl FnOnce() -> Value) -> Option<(Value, String)> {
        let due = self
            .last_uploaded
            .map(|at| at.elapsed() >= UPLOAD_SYSINFO_TIMEOUT)
            .unwrap_or(true);
        if !due {
            return None;
        }
        let info = collect();
        // For Windows:
        // We can't skip uploading sysinfo when the username is empty, because the username
        // may always be empty before login. We also need to upload the other sysinfo info.
        //
        // https://github.com/rustdesk/rustdesk/discussions/8031
        // We still need to check the username after uploading sysinfo, because
        // 1. The username may be empty when logining in, and it can be fetched after a
        //    while. In this case, we need to upload sysinfo again.
        // 2. The username may be changed after uploading sysinfo, and we need to upload
        //    sysinfo again.
        //
        // The Windows session will switch to the last user session before the restart,
        // so it may be able to get the username before login. But strangely, sometimes we
        // can get the username before login, we may not be able to get the username
        // before login after the next restart.
        let username = info["username"].as_str().unwrap_or_default().to_string();
        let changed = !self.uploaded || self.username.as_ref() != Some(&username);
        changed.then_some((info, username))
    }
}

impl Default for InfoUploaded {
    fn default() -> Self {
        Self {
            uploaded: false,
            url: "".to_owned(),
            last_uploaded: None,
            id: "".to_owned(),
            username: None,
        }
    }
}

impl InfoUploaded {
    fn uploaded(url: String, id: String, username: String) -> Self {
        Self {
            uploaded: true,
            url,
            last_uploaded: None,
            id,
            username: Some(username),
        }
    }
}

#[cfg(all(
    test,
    feature = "flutter",
    not(any(target_os = "android", target_os = "ios"))
))]
mod tests {
    use super::{switch_code_verifier, switch_grant_signed_msg};

    #[test]
    fn test_switch_code_verifier_is_not_raw_switch_code() {
        let switch_code = "code-abc";
        let verifier = switch_code_verifier(switch_code);
        assert_ne!(verifier, switch_code);
        assert_eq!(verifier, switch_code_verifier(switch_code));
        assert_eq!(
            verifier,
            "dMIn3uiPe77XodFB5IKi7PrKJ7l7+zVquNn0ObSaHQc="
        );
    }

    #[test]
    fn test_switch_grant_signed_msg_layout() {
        let expected: Vec<u8> = [
            &b"switch-grant\0"[..],
            b"id1",
            b"\0",
            b"c1",
            b"\0",
            b"1700000000",
        ]
        .concat();
        assert_eq!(switch_grant_signed_msg("id1", "c1", "1700000000"), expected);
    }
}
