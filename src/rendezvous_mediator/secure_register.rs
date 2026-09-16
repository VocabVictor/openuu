//! Which transport the controlled peer registers on.
//!
//! On UDP the peer's `RegisterPk` and its heartbeat go out in the clear, and
//! hbbs answers `PunchHole`, `RequestRelay` and `FetchLocalAddr` the same way --
//! the messages that decide where this machine punches and which relay it
//! trusts. The TCP path carries them through `secure_tcp`, and hbbs has
//! registered peers on it since `tcp_register.rs`.
//!
//! Product builds already default to TCP: `builtin.rs` sets `disable-udp` to
//! `Y` whenever an id server is baked in. What is here covers the rest --
//! builds with no baked-in server, and the fallback below.
//!
//! # Why it falls back
//!
//! Upstream's hbbs answers `RegisterPk` on TCP with NOT_SUPPORT and an older
//! OpenUU server ignores it, while `start_all` retries a failed mediator
//! forever. Without a fallback a peer pointed at such a deployment never comes
//! online at all, which is worse than the plaintext it replaces. Per host, per
//! process: a server that learns TCP registration is picked up on the next
//! start rather than held to what it did once.

use super::*;

/// Three states, not a bool: never touched is not the same as turned off.
pub(super) enum Udp {
    /// `Y`: TCP and stays there. No fallback -- the user asked for no UDP.
    Off,
    /// `N`: UDP.
    On,
    /// Unset: TCP, with the fallback.
    Unset,
}

impl Udp {
    pub(super) fn read() -> Self {
        Self::of(&Config::get_option(OPTION_DISABLE_UDP))
    }

    pub(super) fn of(value: &str) -> Self {
        match value {
            "Y" => Self::Off,
            "N" => Self::On,
            _ => Self::Unset,
        }
    }
}

lazy_static::lazy_static! {
    /// Hosts whose hbbs answered no registration on TCP, by `host_prefix`.
    static ref BROKEN: Mutex<std::collections::HashSet<String>> = Default::default();
}

/// Set by `handle_resp` on any answer to a registration -- OK, UUID_MISMATCH
/// and NOT_DEPLOYED all mean the server understood it. Cleared per session.
static ANSWERED: AtomicBool = AtomicBool::new(false);

pub(super) async fn tcp_broken(host: &str) -> bool {
    BROKEN.lock().await.contains(host)
}

pub(super) fn answered() {
    ANSWERED.store(true, Ordering::SeqCst);
}

pub(super) fn session_start() {
    ANSWERED.store(false, Ordering::SeqCst);
}

/// Called when a TCP session ends; answers whether to try UDP now. A session
/// that registered was a working one however it ended, so it never falls back.
pub(super) async fn session_end(host: &str) -> bool {
    if ANSWERED.load(Ordering::SeqCst) {
        return false;
    }
    if BROKEN.lock().await.insert(host.to_owned()) {
        log::warn!("{host} answered no registration over TCP; using UDP for this run");
    }
    true
}

#[cfg(test)]
pub(super) async fn forget() {
    BROKEN.lock().await.clear();
    ANSWERED.store(false, Ordering::SeqCst);
}
