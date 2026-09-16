//! Which transport the controlled peer registers on.
//!
//! # Why TCP is the default
//!
//! On UDP the peer's `RegisterPk` and its `RegisterPeer` heartbeat go out in the clear,
//! and hbbs answers `PunchHole`, `RequestRelay` and `FetchLocalAddr` the same way. Those
//! are the messages that decide where this machine punches and which relay it trusts, so
//! anyone on the path can map the deployment by listening and can point a peer at a relay
//! of their own by forging one. The TCP path carries the same messages through
//! `secure_tcp` -- hbbs signs a KeyExchange with the ID server's private key, the peer
//! seals a symmetric key back, and the rest of the stream is encrypted -- and hbbs has
//! registered peers on it since `tcp_register.rs` (`docs/peer-registration-encryption.md`,
//! option a).
//!
//! # Why it still falls back
//!
//! Upstream's hbbs answers `RegisterPk` on TCP with NOT_SUPPORT, and an OpenUU server
//! older than `tcp_register.rs` ignores it. Neither answers the heartbeat. `start_all`
//! retries a failed mediator forever, so a peer pointed at such a deployment would retry
//! TCP and never come online at all -- a worse failure than the plaintext this replaces.
//! The first session that ends without one registration answer marks the host, and every
//! attempt after it goes back to UDP until the process restarts.
//!
//! The mark is per host and per process rather than stored: a server that learns TCP
//! registration should be picked up on its own, and nothing on this side would notice
//! that it had.

use super::*;

/// What `disable-udp` was set to, as three states rather than a bool: a user who has
/// never touched the switch is not the same as one who turned it off.
pub(super) enum UdpPreference {
    /// Registration goes over TCP and stays there; no fallback, because the user asked.
    TcpOnly,
    /// Registration goes over UDP; the user turned the switch off.
    UdpOnly,
    /// Nothing configured: TCP, with the fallback above.
    Default,
}

impl UdpPreference {
    pub(super) fn read() -> Self {
        Self::of(&Config::get_option(OPTION_DISABLE_UDP))
    }

    pub(super) fn of(value: &str) -> Self {
        match value {
            "Y" => Self::TcpOnly,
            "N" => Self::UdpOnly,
            _ => Self::Default,
        }
    }
}

lazy_static::lazy_static! {
    /// Hosts whose hbbs did not answer a registration on TCP, by `host_prefix`.
    static ref NO_TCP_REGISTER: Mutex<std::collections::HashSet<String>> = Default::default();
}

/// Set by `handle_resp` on any answer to a registration -- OK, UUID_MISMATCH and
/// NOT_DEPLOYED all mean the server understood it, which is what this asks. Cleared at the
/// start of each TCP session, so it describes that session and no other.
static REGISTERED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

/// Whether registration is possible over TCP with this host, as far as we have seen.
pub(super) async fn tcp_register_known_broken(host_prefix: &str) -> bool {
    NO_TCP_REGISTER.lock().await.contains(host_prefix)
}

pub(super) fn registration_answered() {
    REGISTERED_THIS_SESSION.store(true, Ordering::SeqCst);
}

pub(super) fn begin_tcp_session() {
    REGISTERED_THIS_SESSION.store(false, Ordering::SeqCst);
}

/// Called when a TCP session ends. Answers whether the caller should try UDP now.
///
/// A session that registered at least once was a working session however it ended -- a
/// reset, a timeout, a restart -- and says nothing about the server's support, so it is
/// never a reason to fall back.
pub(super) async fn tcp_session_ended(host_prefix: &str) -> bool {
    if REGISTERED_THIS_SESSION.load(Ordering::SeqCst) {
        return false;
    }
    let first = NO_TCP_REGISTER
        .lock()
        .await
        .insert(host_prefix.to_owned());
    if first {
        log::warn!(
            "{host_prefix} answered no registration over TCP; \
             falling back to the plaintext UDP rendezvous for this run"
        );
    }
    true
}

#[cfg(test)]
pub(super) async fn forget_hosts() {
    NO_TCP_REGISTER.lock().await.clear();
    REGISTERED_THIS_SESSION.store(false, Ordering::SeqCst);
}
