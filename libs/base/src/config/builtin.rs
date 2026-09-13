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
/// Set to "Y" whenever an id server is baked in: the peer then registers over the
/// secured TCP rendezvous connection (docs/peer-registration-encryption.md). A
/// user value or a custom client file still overrides it.
pub const KEY_DISABLE_UDP: &str = "disable-udp";

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
    apply_ui_defaults();
    apply_local_defaults();
}

/// Product defaults for this machine's own settings (`LocalConfig`), in the
/// default layer a custom client's `default-settings` uses: a stored user value
/// wins, an empty one falls back to them. On Windows H264/H265 streams decode
/// into a D3D11 texture instead of a CPU YUV->RGBA copy per frame; upstream keeps
/// this off because `allow-*` keys are only true when set to "Y".
pub fn apply_local_defaults() {
    let mut d = config::DEFAULT_LOCAL_SETTINGS.write().unwrap();
    #[cfg(windows)]
    d.insert(crate::config::keys::OPTION_ALLOW_D3D_RENDER.to_owned(), "Y".to_owned());
    let _ = &mut d;
}

/// Product defaults for the remote-control UI, in the same display-settings default
/// layer a custom client file uses (`UserDefaultConfig` reads it after the user's own
/// value): the desktop viewer opens remote screens fitted to the window instead of at
/// 1:1, which upstream only does on mobile.
pub fn apply_ui_defaults() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    config::DEFAULT_DISPLAY_SETTINGS
        .write()
        .unwrap()
        .insert(config::keys::OPTION_VIEW_STYLE.to_owned(), "adaptive".to_owned());
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
        config::DEFAULT_SETTINGS
            .write()
            .unwrap()
            .insert(KEY_DISABLE_UDP.to_owned(), "Y".to_owned());
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
        assert_eq!(defaults(KEY_DISABLE_UDP).as_deref(), Some("Y"));
        // an empty user value falls back to the default
        assert_eq!(config::Config::get_option(KEY_ID_SERVER), "rs.test");
        let mut d = config::DEFAULT_SETTINGS.write().unwrap();
        for k in [KEY_ID_SERVER, KEY_API_SERVER, KEY_KEY, KEY_DISABLE_UDP] {
            d.remove(k);
        }
        drop(d);
        *config::PROD_RENDEZVOUS_SERVER.write().unwrap() = String::new();
    }

    // The user's own value sits above this layer in UserDefaultConfig::get (it is read
    // before DEFAULT_DISPLAY_SETTINGS), so a stored choice keeps winning; setting one
    // here would write the test machine's real user config, so only the default and the
    // overwrite layer above it are exercised.
    #[cfg(windows)]
    #[test]
    fn d3d_render_defaults_on_and_a_local_overwrite_wins() {
        apply_local_defaults();
        let key = crate::config::keys::OPTION_ALLOW_D3D_RENDER;
        assert_eq!(config::LocalConfig::get_option(key), "Y");
        assert!(config::option2bool(key, &config::LocalConfig::get_option(key)));
        config::OVERWRITE_LOCAL_SETTINGS
            .write()
            .unwrap()
            .insert(key.to_owned(), "N".to_owned());
        assert_eq!(config::LocalConfig::get_option(key), "N");
        config::OVERWRITE_LOCAL_SETTINGS.write().unwrap().remove(key);
        config::DEFAULT_LOCAL_SETTINGS.write().unwrap().remove(key);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn desktop_view_style_defaults_to_adaptive() {
        apply_ui_defaults();
        let key = config::keys::OPTION_VIEW_STYLE;
        assert_eq!(
            config::DEFAULT_DISPLAY_SETTINGS.read().unwrap().get(key).map(String::as_str),
            Some("adaptive")
        );
        assert_eq!(config::UserDefaultConfig::load().get(key), "adaptive");
        config::OVERWRITE_DISPLAY_SETTINGS
            .write()
            .unwrap()
            .insert(key.to_owned(), "original".to_owned());
        assert_eq!(config::UserDefaultConfig::load().get(key), "original");
        config::OVERWRITE_DISPLAY_SETTINGS.write().unwrap().remove(key);
        config::DEFAULT_DISPLAY_SETTINGS.write().unwrap().remove(key);
    }
}
