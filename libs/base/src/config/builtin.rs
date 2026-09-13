//! Server defaults baked into the binary at build time
//! (docs/server-config-provisioning.md §2).
//!
//! They sit in the same `DEFAULT_SETTINGS` layer as a custom client's
//! `default-settings`: a user's non-empty value wins, an empty one falls back
//! to them, and `Config::set_option` already refuses to persist a value equal
//! to the default. `PROD_RENDEZVOUS_SERVER`, which `Config::get_rendezvous_server`
//! consults ahead of the built-in public server list, is set to the id server.

use hbb_common::{config, log};

include!(concat!(env!("OUT_DIR"), "/builtin_defaults.rs"));

pub const KEY_ID_SERVER: &str = "custom-rendezvous-server";
pub const KEY_RELAY_SERVER: &str = "relay-server";
pub const KEY_API_SERVER: &str = "api-server";
pub const KEY_KEY: &str = "key";

/// True when the build carried at least an id server.
pub fn is_configured() -> bool {
    !BUILTIN_ID_SERVER.is_empty()
}

/// Installs the baked-in defaults. Called once at start-up before the custom
/// client file is read, so a `custom.txt` can still override them.
pub fn apply() {
    apply_values(
        BUILTIN_ID_SERVER,
        BUILTIN_RELAY_SERVER,
        BUILTIN_API_SERVER,
        BUILTIN_KEY,
    );
}

fn apply_values(id: &str, relay: &str, api: &str, key: &str) {
    let pairs = [
        (KEY_ID_SERVER, id),
        (KEY_RELAY_SERVER, relay),
        (KEY_API_SERVER, api),
        (KEY_KEY, key),
    ];
    let mut applied = 0;
    {
        let mut defaults = config::DEFAULT_SETTINGS.write().unwrap();
        for (k, v) in pairs {
            if !v.is_empty() {
                defaults.insert(k.to_owned(), v.to_owned());
                applied += 1;
            }
        }
    }
    if !id.is_empty() {
        *config::PROD_RENDEZVOUS_SERVER.write().unwrap() = id.to_owned();
    }
    if applied > 0 {
        // The key is never logged; only whether the build carries one.
        log::info!(
            "event=builtin_defaults id_server={} relay={} api={} key={}",
            id,
            relay,
            api,
            if key.is_empty() { "<unset>" } else { "<set>" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults(k: &str) -> Option<String> {
        config::DEFAULT_SETTINGS.read().unwrap().get(k).cloned()
    }

    // One test: the tables are process-global and the two halves must not race.
    #[test]
    fn empty_values_add_nothing_and_values_become_defaults() {
        let before = config::DEFAULT_SETTINGS.read().unwrap().len();
        apply_values("", "", "", "");
        assert_eq!(config::DEFAULT_SETTINGS.read().unwrap().len(), before);
        assert!(config::PROD_RENDEZVOUS_SERVER.read().unwrap().is_empty());

        apply_values("rs.test", "", "https://rs.test:21114", "pk==");
        assert_eq!(defaults(KEY_ID_SERVER).as_deref(), Some("rs.test"));
        assert_eq!(defaults(KEY_RELAY_SERVER), None);
        assert_eq!(defaults(KEY_API_SERVER).as_deref(), Some("https://rs.test:21114"));
        assert_eq!(defaults(KEY_KEY).as_deref(), Some("pk=="));
        assert_eq!(*config::PROD_RENDEZVOUS_SERVER.read().unwrap(), "rs.test");
        // an empty user value falls back to the default
        assert_eq!(config::Config::get_option(KEY_ID_SERVER), "rs.test");
        let mut d = config::DEFAULT_SETTINGS.write().unwrap();
        for k in [KEY_ID_SERVER, KEY_API_SERVER, KEY_KEY] {
            d.remove(k);
        }
        drop(d);
        *config::PROD_RENDEZVOUS_SERVER.write().unwrap() = String::new();
    }
}
