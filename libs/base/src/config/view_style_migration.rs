//! Bringing peers configured before the desktop default changed onto that default.
//!
//! `builtin::apply_ui_defaults` puts `adaptive` in the `DEFAULT_DISPLAY_SETTINGS`
//! layer, and that layer is only consulted for a field a peer's own file does not
//! have. Every peer connected before that default landed has `view_style =
//! "original"` written in its file -- put there by the old default, not chosen --
//! so those sessions keep opening at 1:1 no matter what the default becomes. On a
//! high-DPI screen that is the difference between a remote screen fitted to the
//! window and one drawn at two thirds size in the corner of it.
//!
//! This rewrites them, once. **The flag is the point**: a value written by a
//! default and a value chosen by the user are the same two words in the same
//! file, so the only honest way to correct the first without overriding the
//! second forever is to do it a single time and never look again.

use hbb_common::{
    config::{self, Config, LocalConfig, PeerConfig},
    log,
};

/// Set once the rewrite below has run. Its absence is what makes the rewrite
/// happen, so it is stored even when nothing needed rewriting.
const MIGRATED_FLAG: &str = "view-style-adaptive-migrated";

const OLD_DEFAULT: &str = "original";
const NEW_DEFAULT: &str = "adaptive";

pub fn run_once() {
    if LocalConfig::get_option(MIGRATED_FLAG) == "Y" {
        return;
    }
    // Stored first: a rewrite that dies half way through has still changed
    // whichever peers it reached, and running it again would take away an
    // `original` the user chose in between.
    LocalConfig::set_option(MIGRATED_FLAG.to_owned(), "Y".to_owned());
    let wanted = config::UserDefaultConfig::load().get(config::keys::OPTION_VIEW_STYLE);
    if wanted != NEW_DEFAULT {
        // The user, or a custom client file, asks for something else. Nothing to
        // bring onto a default that is not the default here.
        return;
    }
    let mut changed = 0;
    for (id, _, mut peer) in PeerConfig::peers(None) {
        if !needs_rewrite(&peer.view_style, &wanted) {
            continue;
        }
        peer.view_style = NEW_DEFAULT.to_owned();
        peer.store(&id);
        changed += 1;
    }
    if changed > 0 {
        log::info!("event=view_style_migrated peers={changed} to={NEW_DEFAULT}");
    }
}

/// Whether a peer's stored view style is a leftover of the old default rather
/// than a choice, as far as anything here can tell: it matches the default it
/// was written under, and the default has since moved.
///
/// `current_default` is passed rather than read so this stays a decision about
/// two strings; everything that makes it hard to test lives in `run_once`.
fn needs_rewrite(stored: &str, current_default: &str) -> bool {
    stored == OLD_DEFAULT && current_default == NEW_DEFAULT
}

/// Whether this install has already been through the rewrite. For the tests and
/// for anything that needs to say why a peer was left alone.
pub fn already_run() -> bool {
    LocalConfig::get_option(MIGRATED_FLAG) == "Y"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_old_default_is_rewritten() {
        assert!(needs_rewrite("original", "adaptive"));
        // Already on it, or on something else entirely: nothing to do.
        assert!(!needs_rewrite("adaptive", "adaptive"));
        assert!(!needs_rewrite("custom", "adaptive"));
        assert!(!needs_rewrite("", "adaptive"));
    }

    #[test]
    fn a_deployment_that_wants_original_keeps_every_peer_on_it() {
        // A custom client file or a user default of `original` means the old
        // value is the wanted one, so the rewrite must not fire at all --
        // otherwise this "migration" would fight the configured default once
        // per install.
        assert!(!needs_rewrite("original", "original"));
        assert!(!needs_rewrite("adaptive", "original"));
    }
}

/// The config directory has to exist before peers can be read; `Config::init`
/// does that, and this is here so a caller cannot forget the order.
pub fn run_once_after_init() {
    if Config::get_id().is_empty() {
        // No identity yet means a fresh install: there are no old peers to bring
        // forward, and the flag would only hide a later migration from a peer
        // file restored from a backup.
        return;
    }
    run_once();
}
