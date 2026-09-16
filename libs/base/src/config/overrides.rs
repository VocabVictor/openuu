//! What this run says, over what the machine has stored.
//!
//! # The order, and why it is this one
//!
//! ```text
//! OPENUU_OPT_<KEY>   environment   highest -- this run, this shell
//! --option <k>=<v>   command line
//! <user's config>    stored files  -- what the app writes when you change a setting
//! openuu-config.json provisioning  -- a deployment's defaults
//! built-in           baked in      lowest  -- builtin.rs
//! ```
//!
//! The two at the top exist to answer one question without editing anything:
//! *what does this build do if I change this one value*. They are the shortest
//! way to find out, so they win over what is on disk; nothing they set is
//! written anywhere, and the next start without them is the old behaviour.
//!
//! **They also win over a provisioned `locked` key**, which is the one choice
//! here worth arguing about. A lock is a deployment's policy, and this lets a
//! person with a shell on the machine step over it. That is the honest reading
//! of what a lock can do: it is a fence in the UI, not a security boundary --
//! anyone who can set an environment variable for the process can also edit the
//! configuration file it reads, or run a build of their own.
//!
//! # What they cannot reach
//!
//! They are applied once, after the command line is parsed, so anything read
//! before that -- `global_init`, the built-in defaults, importing a
//! provisioning file -- sees the stored values. Environment variables reach a
//! child process because it inherits them; `--option` does not, because the
//! arguments are not passed on.
//!
//! # Why applying and reporting are two steps
//!
//! They have to be applied before anything reads a setting, and the logger is
//! not up that early: `core_main` calls `init_log` well after this. Logging
//! inside `apply` would have written every line into a logger that discards
//! it, which is worse than not logging at all -- the promise in this file that
//! "every override is logged" would have been false and nothing would have
//! said so. So `apply_all` returns what it did and the caller reports it once
//! the log exists.

use super::{keys, provision};
use hbb_common::{config, log};

/// `OPENUU_OPT_ALLOW_D3D_RENDER=N` sets `allow-d3d-render`.
pub const ENV_PREFIX: &str = "OPENUU_OPT_";
/// `--option allow-d3d-render=N`, repeatable.
pub const CLI_FLAG: &str = "--option";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    CommandLine,
    Environment,
}

impl Source {
    fn as_str(&self) -> &'static str {
        match self {
            Source::CommandLine => "cli",
            Source::Environment => "env",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub key: String,
    pub value: String,
    pub source: Source,
}

/// The environment variable that sets `key`: upper case, `-` becomes `_`.
///
/// Only ever used in this direction. Going the other way is ambiguous --
/// `allow-d3d-render` and `allow_d3d_render` produce the same variable -- so
/// the environment is read by asking each known key whether its variable is
/// set, never by parsing a variable name into a key.
pub fn env_name(key: &str) -> String {
    format!("{ENV_PREFIX}{}", key.to_uppercase().replace('-', "_"))
}

/// Every key that can be overridden, which is every key the provisioning file
/// can carry. A key outside these tables has no table to be written into.
fn known_keys() -> impl Iterator<Item = &'static str> {
    keys::KEYS_SETTINGS
        .iter()
        .chain(keys::KEYS_DISPLAY_SETTINGS.iter())
        .chain(keys::KEYS_LOCAL_SETTINGS.iter())
        .copied()
}

/// Splits `--option k=v` out of the arguments. Returns what was found and the
/// arguments with those removed, so the caller hands the rest on unchanged.
pub fn take_from_command_line(args: Vec<String>) -> (Vec<Override>, Vec<String>) {
    let mut found = Vec::new();
    let mut rest = Vec::new();
    let mut expecting = false;
    for arg in args {
        if expecting {
            expecting = false;
            push_pair(&mut found, &arg, Source::CommandLine);
            continue;
        }
        if arg == CLI_FLAG {
            expecting = true;
        } else if let Some(pair) = arg.strip_prefix(&format!("{CLI_FLAG}=")) {
            push_pair(&mut found, pair, Source::CommandLine);
        } else {
            rest.push(arg);
        }
    }
    if expecting {
        log::warn!("event=config_override_bad arg={CLI_FLAG} reason=missing_value");
    }
    (found, rest)
}

fn push_pair(out: &mut Vec<Override>, pair: &str, source: Source) {
    match pair.split_once('=') {
        Some((k, v)) if !k.is_empty() => out.push(Override {
            key: k.to_owned(),
            value: v.to_owned(),
            source,
        }),
        _ => log::warn!(
            "event=config_override_bad value={pair} reason=not_key_equals_value"
        ),
    }
}

/// Reads one variable per known key. Nothing is parsed out of the environment's
/// own names, so a variable that looks like ours but matches no key is ignored
/// in silence -- it is far more likely to be someone else's than a typo of ours.
pub fn from_environment() -> Vec<Override> {
    let mut found = Vec::new();
    for key in known_keys() {
        if let Ok(value) = std::env::var(env_name(key)) {
            found.push(Override {
                key: key.to_owned(),
                value,
                source: Source::Environment,
            });
        }
    }
    found
}

/// What `apply_all` did, to be logged once the logger exists.
#[derive(Debug, Default)]
pub struct Report {
    pub applied: Vec<Override>,
    pub refused: Vec<(Override, &'static str)>,
}

impl Report {
    /// Called after `init_log`. The values are logged: these are the keys a
    /// deployment file may carry, and knowing what a run was told is the whole
    /// point of having said it here.
    pub fn log(&self) {
        for o in &self.applied {
            log::info!(
                "event=config_override key={} value={} source={}",
                o.key,
                o.value,
                o.source.as_str()
            );
        }
        for (o, why) in &self.refused {
            log::warn!(
                "event=config_override_refused key={} source={} reason={why}",
                o.key,
                o.source.as_str()
            );
        }
    }
}

/// Command line first, environment second, so the environment wins where both
/// name the same key: they are written into one table and the last write is the
/// one that stays.
pub fn apply_all(from_cli: Vec<Override>) -> Report {
    let mut report = Report::default();
    for o in from_cli.into_iter().chain(from_environment()) {
        match apply_one(&o) {
            None => report.applied.push(o),
            Some(why) => report.refused.push((o, why)),
        }
    }
    report
}

/// `None` when it was applied, `Some(reason)` when it was not.
fn apply_one(o: &Override) -> Option<&'static str> {
    // Secrets have their own stores and are never a table entry; an override
    // that set one would also be the easiest way to leave a password in a shell
    // history or a process listing.
    if provision::is_secret_key(&o.key) {
        return Some("secret");
    }
    let table = if provision::is_setting(&o.key) {
        &*config::OVERWRITE_SETTINGS
    } else if provision::is_display_setting(&o.key) {
        &*config::OVERWRITE_DISPLAY_SETTINGS
    } else if provision::is_local_setting(&o.key) {
        &*config::OVERWRITE_LOCAL_SETTINGS
    } else {
        return Some("no such key");
    };
    table
        .write()
        .unwrap()
        .insert(o.key.clone(), o.value.clone());
    None
}

#[cfg(test)]
mod tests;
