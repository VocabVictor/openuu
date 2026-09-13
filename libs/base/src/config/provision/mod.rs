//! Configuration provisioning: `openuu-config.json`, the clipboard / QR / deep
//! link payloads and the legacy RustDesk server-config string
//! (docs/server-config-provisioning.md §3–4).
//!
//! `parse` turns any of those texts into a [`Provision`], `validate` checks it,
//! `apply::apply` installs it into the same tables a custom client file uses,
//! and `encode_share` produces the QR / deep-link payload.

mod apply;

use super::keys;
use hbb_common::{
    bail,
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine},
    ResultType,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use apply::{
    apply, import_exe_dir_file, reload_persisted, EXE_DIR_FILE, HASH_KEY, PERSISTED_FILE,
};

pub const VERSION: u32 = 1;
/// Prefix of the QR / deep-link payload; the rest is base64url (no padding) JSON.
pub const URI_PREFIX: &str = "openuu://config/";
/// Legacy mobile scanner prefix in front of the RustDesk server-config string.
pub const LEGACY_PREFIX: &str = "config=";
/// Hard cap for the encoded share payload: a version-20 QR at level M scans
/// reliably on phones.
pub const QR_MAX_BYTES: usize = 1200;

/// The `server` section is sugar for these option keys.
pub const SERVER_KEYS: [(&str, &str); 4] = [
    ("id", super::builtin::KEY_ID_SERVER),
    ("relay", super::builtin::KEY_RELAY_SERVER),
    ("api", super::builtin::KEY_API_SERVER),
    ("key", super::builtin::KEY_KEY),
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub relay: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provision {
    /// Mandatory; a file without it is not a provisioning file.
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub server: Server,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub local: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locked: Vec<String>,
}

/// Where a text came from. Only file-like sources may lock keys or carry secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// `openuu-config.json` next to the executable or the persisted copy.
    File,
    /// `--import-config <path>.json`, including the MSI's temporary service.
    Cli,
    /// A file the user picked in the settings page.
    Picked,
    /// QR code, deep link or clipboard: unauthenticated text.
    Untrusted,
}

impl Source {
    pub fn trusted(self) -> bool {
        !matches!(self, Source::Untrusted)
    }
}

/// What an import did, by key name only; secret values are never included.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub applied: Vec<String>,
    pub locked: Vec<String>,
    pub secrets: Vec<String>,
    pub ignored: Vec<String>,
    pub warnings: Vec<String>,
}

/// Keys whose values are secrets: applied to their own stores, never to the
/// default tables, never logged or echoed.
pub fn is_secret_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.ends_with("password") || k.ends_with("pin") || k.ends_with("token")
}

/// Accepts the JSON file, the QR / deep-link payload (with or without the
/// `openuu://config/` prefix), the legacy mobile `config=` form and the legacy
/// RustDesk clipboard string (reversed base64url of `{host, relay, api, key}`).
pub fn parse(text: &str) -> ResultType<Provision> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix(URI_PREFIX) {
        return parse_encoded(rest.trim_end_matches('/'));
    }
    if let Some(rest) = text.strip_prefix(LEGACY_PREFIX) {
        return parse_legacy(rest);
    }
    if text.starts_with('{') {
        return parse_json(text);
    }
    parse_encoded(text).or_else(|_| parse_legacy(text))
}

fn parse_json(text: &str) -> ResultType<Provision> {
    let p: Provision = serde_json::from_str(text)?;
    if p.version == 0 {
        bail!("missing version");
    }
    Ok(p)
}

fn parse_encoded(text: &str) -> ResultType<Provision> {
    let bytes = URL_SAFE_NO_PAD.decode(text.trim_end_matches('='))?;
    let json = String::from_utf8(bytes)?;
    parse_json(json.trim())
}

#[derive(Deserialize)]
struct Legacy {
    #[serde(default)]
    host: String,
    #[serde(default)]
    relay: String,
    #[serde(default)]
    api: String,
    #[serde(default)]
    key: String,
}

fn parse_legacy(text: &str) -> ResultType<Provision> {
    let reversed: String = text.trim().chars().rev().collect();
    let bytes = URL_SAFE_NO_PAD.decode(reversed.trim_end_matches('='))?;
    let legacy: Legacy = serde_json::from_slice(&bytes)?;
    if legacy.host.is_empty() {
        bail!("legacy server config without a host");
    }
    Ok(Provision {
        version: VERSION,
        server: Server {
            id: legacy.host,
            relay: legacy.relay,
            api: legacy.api,
            key: legacy.key,
        },
        ..Default::default()
    })
}

pub fn validate(p: &Provision) -> ResultType<()> {
    if p.version > VERSION {
        bail!(
            "configuration version {} is newer than this OpenUU understands ({})",
            p.version,
            VERSION
        );
    }
    for (_, key) in SERVER_KEYS {
        if p.options.contains_key(key) {
            bail!("{key} is given both in server and in options");
        }
    }
    let api = p.server.api.trim();
    if !api.is_empty() && !(api.starts_with("http://") || api.starts_with("https://")) {
        bail!("server.api must start with http:// or https://");
    }
    for key in &p.locked {
        if !p.options.contains_key(key)
            && !SERVER_KEYS.iter().any(|(_, k)| k == key && !server_value(&p.server, k).is_empty())
        {
            bail!("locked key {key} has no value in server or options");
        }
    }
    Ok(())
}

fn server_value<'a>(s: &'a Server, key: &str) -> &'a str {
    match key {
        k if k == SERVER_KEYS[0].1 => &s.id,
        k if k == SERVER_KEYS[1].1 => &s.relay,
        k if k == SERVER_KEYS[2].1 => &s.api,
        k if k == SERVER_KEYS[3].1 => &s.key,
        _ => "",
    }
}

/// The `server` section as option key/value pairs (empty fields skipped).
pub fn server_options(p: &Provision) -> Vec<(String, String)> {
    SERVER_KEYS
        .iter()
        .map(|(_, k)| (k.to_string(), server_value(&p.server, k).trim().to_owned()))
        .filter(|(_, v)| !v.is_empty())
        .collect()
}

/// The QR / deep-link payload: `server` plus the non-secret `options`, no
/// `local`, no `locked`; refused when it would not fit a scannable code.
pub fn encode_share(p: &Provision) -> ResultType<String> {
    let share = Provision {
        version: VERSION,
        server: p.server.clone(),
        options: p
            .options
            .iter()
            .filter(|(k, _)| !is_secret_key(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        ..Default::default()
    };
    let json = serde_json::to_string(&share)?;
    let encoded = format!("{URI_PREFIX}{}", URL_SAFE_NO_PAD.encode(json));
    if encoded.len() > QR_MAX_BYTES {
        bail!(
            "configuration is too large for a QR code ({} of {} bytes); share it as a file",
            encoded.len(),
            QR_MAX_BYTES
        );
    }
    Ok(encoded)
}

/// The file as stored after an import: everything except secret values.
pub fn sanitized(p: &Provision) -> Provision {
    let mut copy = p.clone();
    copy.options.retain(|k, _| !is_secret_key(k));
    copy.local.retain(|k, _| !is_secret_key(k));
    copy
}

pub(crate) fn is_setting(key: &str) -> bool {
    keys::KEYS_SETTINGS.contains(&key)
}
pub(crate) fn is_display_setting(key: &str) -> bool {
    keys::KEYS_DISPLAY_SETTINGS.contains(&key)
}
pub(crate) fn is_local_setting(key: &str) -> bool {
    keys::KEYS_LOCAL_SETTINGS.contains(&key)
}
pub(crate) fn is_buildin_setting(key: &str) -> bool {
    keys::KEYS_BUILDIN_SETTINGS.contains(&key)
}

#[cfg(test)]
mod tests;
