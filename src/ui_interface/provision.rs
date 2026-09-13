//! Flutter-facing configuration import and share
//! (docs/server-config-provisioning.md §3–4). Both return a JSON object so the
//! UI gets the report or the error without a second call.

use base::config::provision::{self, Connect, Provision, Server, Source};
use hbb_common::config::Config;
use serde_json::json;

/// Applies `text` (file content, clipboard, QR or deep-link payload).
/// `trusted` is true for a file the user picked, false for anything that
/// arrived as text; untrusted sources can neither lock keys nor set secrets.
pub fn import_config_text(text: String, trusted: bool) -> String {
    let source = if trusted { Source::Picked } else { Source::Untrusted };
    match provision::apply(&text, source) {
        Ok(report) => json!({"ok": true, "report": report}).to_string(),
        Err(err) => json!({"ok": false, "error": err.to_string()}).to_string(),
    }
}

/// Decodes without applying, for the confirmation dialog: the server values
/// and the option key names the payload would set.
pub fn preview_config_text(text: String) -> String {
    match provision::parse(&text).and_then(|p| {
        provision::validate(&p)?;
        Ok(p)
    }) {
        Ok(p) => json!({
            "ok": true,
            "server": p.server,
            "options": p.options.keys().collect::<Vec<_>>(),
            "locked": p.locked,
            "connect": p.connect,
        })
        .to_string(),
        Err(err) => json!({"ok": false, "error": err.to_string()}).to_string(),
    }
}

/// The QR / deep-link payload for the current server settings plus the given
/// option keys (secrets are dropped, the size cap applies).
pub fn encode_share_config(option_keys: Vec<String>) -> String {
    encode_share_config_with_connect(option_keys, String::new(), String::new())
}

/// Same payload plus the device the receiver should connect to (assistance page).
pub fn encode_share_config_with_connect(option_keys: Vec<String>, id: String, password: String) -> String {
    let p = Provision {
        version: provision::VERSION,
        server: Server {
            id: Config::get_option(base::config::builtin::KEY_ID_SERVER),
            relay: Config::get_option(base::config::builtin::KEY_RELAY_SERVER),
            api: Config::get_option(base::config::builtin::KEY_API_SERVER),
            key: Config::get_option(base::config::builtin::KEY_KEY),
        },
        options: option_keys
            .into_iter()
            .map(|k| {
                let v = Config::get_option(&k);
                (k, v)
            })
            .filter(|(_, v)| !v.is_empty())
            .collect(),
        connect: (!id.is_empty()).then(|| Connect { id, password }),
        ..Default::default()
    };
    match provision::encode_share(&p) {
        Ok(payload) => json!({"ok": true, "payload": payload}).to_string(),
        Err(err) => json!({"ok": false, "error": err.to_string()}).to_string(),
    }
}
