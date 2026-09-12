use std::{
    collections::{hash_map::RandomState, HashMap, VecDeque},
    hash::BuildHasher,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use uuid::Uuid;

use hbb_common::{
    allow_err,
    anyhow::{self, bail},
    config::{self, option2bool, use_ws, Config, CONNECT_TIMEOUT, REG_INTERVAL, RENDEZVOUS_PORT},
    futures::future::join_all,
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep,
    socket_client::{self, connect_tcp, is_ipv4, new_direct_udp_for, new_udp_for},
    tokio::{
        self, select,
        sync::{mpsc, Mutex},
        time::interval,
    },
    udp::FramedSocket,
    webrtc::WebRTCStream,
    AddrMangle, IntoTargetAddr, ResultType, Stream, TargetAddr,
};
use base::config::keys::*;

use crate::{
    check_port,
    server::{check_zombie, new as new_server, ConnectionMeta, ServerPtr},
};

mod lifecycle;
mod udp;
mod relay;
mod webrtc;
mod punch;
mod register;
mod direct;
mod tcp_punch;
#[cfg(test)]
mod tests;
use direct::{direct_server, start_ipv6, udp_nat_listen};
use tcp_punch::punch_tcp_until_connected;
#[cfg(test)]
use tcp_punch::{punch_until, PUNCH_GRACE, PUNCH_INTERVAL, PUNCH_MAX_INTERVAL};

type Message = RendezvousMessage;

fn connection_meta(
    control_permissions: Option<ControlPermissions>,
    controlled_context: Option<ControlledContext>,
) -> ConnectionMeta {
    ConnectionMeta {
        control_permissions,
        controlled_context,
    }
}

lazy_static::lazy_static! {
    static ref SOLVING_PK_MISMATCH: Mutex<String> = Default::default();
    static ref LAST_MSG: Mutex<(SocketAddr, Instant)> = Mutex::new((SocketAddr::new([0; 4].into(), 0), Instant::now()));
    static ref LAST_RELAY_MSG: Mutex<(SocketAddr, Instant)> = Mutex::new((SocketAddr::new([0; 4].into(), 0), Instant::now()));
    static ref WEBRTC_ICE_TXS: Mutex<HashMap<String, IceRoute>> = Default::default();
    static ref ICE_DIGEST_STATE: RandomState = Default::default();
}
/// Remote ICE candidates buffered per session while the answerer applies them. Same depth as the
/// controller's own buffer (`Client::MAX_PENDING_WEBRTC_ICE`), though that one evicts its oldest
/// where a full channel here refuses the newest.
const MAX_PENDING_REMOTE_ICE: usize = 64;
/// Queued candidates remembered so the controller's re-send is skipped instead of taking a slot
/// of its own. Far more than an honest peer gathers, at eight bytes each.
const ICE_DEDUP_WINDOW: usize = 256;
// The rendezvous ICE route is reachable without a prior punch and the peer decides how many
// candidates it sends, so these sites would let someone else set how much this machine writes to
// its log file. One line a minute each, carrying the suppressed count.
const ICE_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
static UNKNOWN_ICE_SESSION_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(ICE_LOG_INTERVAL);
static REJECTED_REMOTE_ICE_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(ICE_LOG_INTERVAL);
static FULL_ICE_QUEUE_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(ICE_LOG_INTERVAL);

struct IceRoute {
    tx: mpsc::Sender<String>,
    recent: VecDeque<u64>,
}

impl IceRoute {
    fn new(tx: mpsc::Sender<String>) -> Self {
        Self {
            tx,
            recent: VecDeque::new(),
        }
    }

    /// Keeps `queue` the only way onto the channel, so nothing reaches it unrecorded.
    fn is_same_channel(&self, other: &mpsc::Sender<String>) -> bool {
        self.tx.same_channel(other)
    }

    /// Skip the controller's re-send of a candidate already queued: the ICE agent that dedups
    /// repeats is downstream of this queue, so the copy would spend a slot of its own.
    /// False means the candidate was dropped.
    fn queue(&mut self, candidate: String) -> bool {
        let digest = ICE_DIGEST_STATE.hash_one(candidate.as_str());
        if self.recent.contains(&digest) {
            // Only honest about the drop if the route is still alive to have taken it.
            return !self.tx.is_closed();
        }
        // Recorded once queued, never before: a refused candidate stays repairable by the re-send.
        if self.tx.try_send(candidate).is_err() {
            return false;
        }
        if self.recent.len() >= ICE_DEDUP_WINDOW {
            self.recent.pop_front();
        }
        self.recent.push_back(digest);
        true
    }
}

static SHOULD_EXIT: AtomicBool = AtomicBool::new(false);
static MANUAL_RESTARTED: AtomicBool = AtomicBool::new(false);
static SENT_REGISTER_PK: AtomicBool = AtomicBool::new(false);
pub(crate) static NEEDS_DEPLOY: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "android")]
static NOTIFIED_NEEDS_DEPLOY: AtomicBool = AtomicBool::new(false);
// register_pk retry interval (ms) when device is awaiting deployment
const DEPLOY_RETRY_INTERVAL: i64 = 30_000;
lazy_static::lazy_static! {
    static ref LAST_NOT_DEPLOYED_REGISTER: Mutex<Option<Instant>> = Mutex::new(None);
}

// Single source of truth for the "awaiting deployment" backoff. The server has
// already told us this device is not in its db; until the operator runs
// `rustdesk --deploy --token <api_token>` there is no point re-running the
// register path more often than DEPLOY_RETRY_INTERVAL. Gating in the timer
// loops (rather than only inside register_pk) also avoids the
// last_register_sent / fails / latency / UDP-rebind churn the loop would
// otherwise spin on while no response ever comes back.
async fn deploy_register_throttled() -> bool {
    if !NEEDS_DEPLOY.load(Ordering::SeqCst) {
        return false;
    }
    LAST_NOT_DEPLOYED_REGISTER
        .lock()
        .await
        .map(|t| (t.elapsed().as_millis() as i64) < DEPLOY_RETRY_INTERVAL)
        .unwrap_or(false)
}

#[cfg(target_os = "android")]
fn notify_android_needs_deploy() {
    if NOTIFIED_NEEDS_DEPLOY.load(Ordering::SeqCst) {
        return;
    }
    let event = serde_json::json!({ "name": "android_needs_deploy" }).to_string();
    if matches!(
        crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, event),
        Some(true)
    ) {
        NOTIFIED_NEEDS_DEPLOY.store(true, Ordering::SeqCst);
    }
}

#[cfg(target_os = "android")]
pub(crate) fn reset_needs_deploy_notification() {
    NEEDS_DEPLOY.store(false, Ordering::SeqCst);
    NOTIFIED_NEEDS_DEPLOY.store(false, Ordering::SeqCst);
}

#[derive(Clone)]
pub struct RendezvousMediator {
    addr: TargetAddr<'static>,
    host: String,
    host_prefix: String,
    keep_alive: i32,
}

enum Sink<'a> {
    Framed(&'a mut FramedSocket, &'a TargetAddr<'a>),
    Stream(&'a mut Stream),
}

impl Sink<'_> {
    async fn send(self, msg: &Message) -> ResultType<()> {
        match self {
            Sink::Framed(socket, addr) => socket.send(msg, addr.to_owned()).await,
            Sink::Stream(stream) => stream.send(msg).await,
        }
    }
}

// When config is not yet synced from root, register_pk may have already been sent with a new generated pk.
// After config sync completes, the pk may change. This struct detects pk changes and triggers
// a re-registration by setting key_confirmed to false.
// NOTE:
// This only corrects PK registration for the current ID. If root uses a non-default mac-generated ID,
// this does not resolve the multi-ID issue by itself.
pub struct CheckIfResendPk {
    pk: Option<Vec<u8>>,
}
impl CheckIfResendPk {
    pub fn new() -> Self {
        Self {
            pk: Config::get_cached_pk(),
        }
    }
}
impl Drop for CheckIfResendPk {
    fn drop(&mut self) {
        if SENT_REGISTER_PK.load(Ordering::SeqCst) && Config::get_cached_pk() != self.pk {
            Config::set_key_confirmed(false);
            log::info!("Set key_confirmed to false due to pk changed, will resend register_pk");
        }
    }
}

