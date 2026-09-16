//! One-time corrections to stored config that a changed default cannot reach.
//!
//! **Do not add a second one.** A changed default reaches configurations that
//! do not have the key yet; that is the rule (`docs/config-precedence.md`).
//! This exists because `view_style` was already written into every peer file by
//! the old default, so no default could reach it.

use hbb_common::{
    config::{self, LocalConfig, PeerConfig},
    log,
};

const FLAG: &str = "view-style-migrated";
const OLD: &str = "original";
const NEW: &str = "adaptive";

/// Peers written before the desktop default became `adaptive` carry `original`
/// in their own file, which no default layer can reach.
///
/// **Call after `load_custom_client` and after `init_log`.** Before the first,
/// the app name is still upstream's and every config path points at RustDesk's
/// directory: an earlier version of this ran in `at_startup`, read an empty peer
/// list from there and wrote its flag there too. Before the second, the log
/// lines go nowhere.
pub fn view_style() {
    if LocalConfig::get_option(FLAG) == "Y" {
        return;
    }
    // Stored first: a run that dies half way has still changed some peers, and
    // a second run would take away an `original` chosen in between.
    LocalConfig::set_option(FLAG.to_owned(), "Y".to_owned());
    if config::UserDefaultConfig::load().get(config::keys::OPTION_VIEW_STYLE) != NEW {
        return; // a deployment that wants `original` keeps it
    }
    let mut n = 0;
    for (id, _, mut peer) in PeerConfig::peers(None) {
        if peer.view_style != OLD {
            continue;
        }
        peer.view_style = NEW.to_owned();
        peer.store(&id);
        n += 1;
    }
    if n > 0 {
        log::info!("event=view_style_migrated peers={n} to={NEW}");
    }
}
