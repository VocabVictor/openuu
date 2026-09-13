use super::*;

mod handle;
mod run;
use run::tunnel_loop;

/// How a tunnel asks whether the account is still signed in.
///
/// A tunnel ends itself when the answer is no. It is handed in rather than called
/// directly so that the ending can be tested: the real check needs an account server and
/// a token, and without them it refuses immediately, which killed every tunnel in the
/// tests before they could show any behaviour of their own.
pub type LoginCheck = std::sync::Arc<
    dyn Fn() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = hbb_common::ResultType<()>> + Send>,
        > + Send
        + Sync,
>;

/// The check production uses.
pub fn account_login_check() -> LoginCheck {
    std::sync::Arc::new(|| Box::pin(crate::account::require_login()))
}
use crate::client::Interface;
use hbb_common::{
    config::READ_TIMEOUT,
    protobuf::Message as _,
    tokio::net::TcpStream,
    Stream,
};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicI32, Ordering},
    time::Duration,
};

/// A refused reason is shown once, then not again until it has been quiet
/// for this long: a page load's dozen refusals make one dialog, and a
/// target that breaks again hours later is reported again.
const REPORT_AGAIN_AFTER: Duration = Duration::from_secs(10);
/// Distinct reasons remembered at once, so the dialog count stays bounded
/// however the peer varies the text it sends.
pub(super) const MAX_REPORTED_OPEN_ERRORS: usize = 8;

// Internal state only; `Claim` is the API listeners see.
enum TunnelState {
    Unset,
    Muxed(Arc<TunnelHandle>),
    Legacy,
    Failed,
}

pub enum Claim {
    Claimed,
    Muxed(Arc<TunnelHandle>),
    Legacy,
}

/// One per listener. The accept loop owns it and reads it between
/// accepts; the tunnel loop resets it when it ends, so the next accept
/// establishes again.
pub struct Tunnel {
    state: watch::Sender<TunnelState>,
    /// Never sent on. The loop's receiver errors when the listener drops
    /// this `Tunnel`, and that is what ends a tunnel nothing else ends.
    lifetime: watch::Sender<()>,
}

impl Tunnel {
    pub fn new() -> Self {
        let (state, _) = watch::channel(TunnelState::Unset);
        let (lifetime, _) = watch::channel(());
        Self { state, lifetime }
    }

    pub fn claim(&self) -> Claim {
        match &*self.state.borrow() {
            TunnelState::Unset | TunnelState::Failed => Claim::Claimed,
            TunnelState::Muxed(h) => Claim::Muxed(h.clone()),
            TunnelState::Legacy => Claim::Legacy,
        }
    }

    pub fn set_muxed(&self, stream: Stream, interface: impl Interface) -> Arc<TunnelHandle> {
        self.set_muxed_checking(stream, interface, account_login_check())
    }

    /// `set_muxed` with the account check handed in.
    ///
    /// The tunnel ends itself when the account is no longer signed in, and that is a
    /// behaviour worth testing rather than only shipping: with the check wired directly to
    /// the account module it could not be exercised at all, because a test has no account
    /// and the tunnel died before any of its own behaviour could be observed. Production
    /// calls `set_muxed`, which passes the real check, so that path is unchanged.
    pub fn set_muxed_checking(
        &self,
        mut stream: Stream,
        interface: impl Interface,
        login_check: LoginCheck,
    ) -> Arc<TunnelHandle> {
        cap_packet_size(&mut stream);
        let (data_tx, data_rx) = mpsc::channel(DATA_QUEUE_FRAMES);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let handle = Arc::new(TunnelHandle {
            sink: FrameSink::Queued { data: data_tx, control: control_tx },
            channels: Mutex::new(HashMap::new()),
            next_id: AtomicI32::new(1),
            reported: Default::default(),
            teardown: watch::channel(false).0,
        });
        let state = self.state.clone();
        // Publish before spawning: if the loop exits first and resets the
        // state, a later publish here would pin it at Muxed with a dead
        // handle and the listener could never re-establish.
        self.state.send_replace(TunnelState::Muxed(handle.clone()));
        tokio::spawn(tunnel_loop(
            stream,
            handle.clone(),
            data_rx,
            control_rx,
            interface,
            state,
            self.lifetime.subscribe(),
            login_check,
        ));
        handle
    }

    pub fn set_legacy(&self) {
        self.state.send_replace(TunnelState::Legacy);
    }

    pub fn set_failed(&self) {
        self.state.send_replace(TunnelState::Failed);
    }
}

struct ChannelEntry {
    inbound: mpsc::UnboundedSender<Inbound>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    opened: bool,
}

pub struct TunnelHandle {
    sink: FrameSink,
    channels: Mutex<HashMap<i32, ChannelEntry>>,
    next_id: AtomicI32,
    reported: Mutex<HashMap<String, Instant>>,
    /// Raised once, by `close_all`, for the channels its `clear` cannot
    /// reach: one parked on its local socket is not on the inbound queue.
    teardown: watch::Sender<bool>,
}
