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

#[cfg(test)]
mod tests {
    use super::{mpsc, socket_client, tokio, IceRoute, ICE_DEDUP_WINDOW, MAX_PENDING_REMOTE_ICE};
    use hbb_common::tcp::new_listener;
    use std::net::SocketAddr;

    // A SOCKS proxy makes `connect_tcp_local` dial the proxy and ignore the local address, so
    // nothing these two assert can hold. Read once, from the same global config production reads.
    fn proxied() -> bool {
        hbb_common::config::Config::get_socks().is_some()
    }

    /// Both held while their addresses are read, so the pair cannot be the same port - which
    /// `SO_REUSEPORT` would let bind twice rather than refuse, leaving the tests degenerate.
    async fn free_loopback_pair() -> (SocketAddr, SocketAddr) {
        let (a, b) = (
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(),
        );
        (a.local_addr().unwrap(), b.local_addr().unwrap())
    }

    fn queue(route: &mut IceRoute, candidate: &str) -> bool {
        route.queue(candidate.to_owned())
    }

    #[test]
    fn the_re_sent_copy_does_not_spend_a_queue_slot() {
        // Two slots, three sends: without the dedup the re-send takes the second and "relay",
        // the one that traverses NAT, is the one refused.
        let (tx, mut rx) = mpsc::channel::<String>(2);
        let mut route = IceRoute::new(tx);
        for _ in 0..2 {
            assert!(queue(&mut route, "host"));
        }
        assert!(queue(&mut route, "relay"));
        let mut queued = Vec::new();
        while let Ok(candidate) = rx.try_recv() {
            queued.push(candidate);
        }
        assert_eq!(queued, vec!["host".to_owned(), "relay".to_owned()]);
    }

    #[test]
    fn a_candidate_the_full_queue_refused_is_not_remembered() {
        let (tx, mut rx) = mpsc::channel::<String>(1);
        let mut route = IceRoute::new(tx);
        assert!(queue(&mut route, "host"));
        assert!(!queue(&mut route, "relay"));
        // The re-send is the only repair for a refused candidate; remembering it would swallow it.
        assert_eq!(rx.try_recv().ok(), Some("host".to_owned()));
        assert!(queue(&mut route, "relay"));
        assert_eq!(rx.try_recv().ok(), Some("relay".to_owned()));
    }

    #[test]
    fn a_re_send_is_skipped_while_the_original_is_still_queued() {
        let (tx, mut rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
        let mut route = IceRoute::new(tx);
        for i in 0..MAX_PENDING_REMOTE_ICE {
            assert!(queue(&mut route, &format!("candidate-{}", i)));
        }
        assert!(queue(&mut route, "candidate-0"));
        let mut queued = 0;
        while rx.try_recv().is_ok() {
            queued += 1;
        }
        assert_eq!(queued, MAX_PENDING_REMOTE_ICE);
    }

    #[test]
    fn the_window_forgets_in_arrival_order() {
        let (tx, mut rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
        let mut route = IceRoute::new(tx);
        for i in 0..=ICE_DEDUP_WINDOW {
            assert!(queue(&mut route, &format!("candidate-{}", i)));
            assert!(rx.try_recv().is_ok());
        }
        // The oldest digest made room for the newest, so its re-send is admitted again.
        assert!(queue(&mut route, "candidate-0"));
        assert!(rx.try_recv().is_ok());
        // A recent one is still skipped.
        let recent = format!("candidate-{}", ICE_DEDUP_WINDOW);
        assert!(queue(&mut route, &recent));
        assert!(rx.try_recv().is_err());
    }

    // The second way in that the repeat punch opens: a punch reaching a peer already in SYN_SENT
    // is answered by that socket rather than reset, and the two ends come up on one connection.
    // A punch that misses the crossing is reset outright here, loopback having no NAT to absorb
    // it and no round trip to hide behind - so a single punch lands only by luck, and repeating
    // is what makes it land at all. That is the premise of the repeat, asserted directly. A round
    // that misses costs one loopback RST, so rounds are cheap and there are many.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_punch_that_meets_the_peers_syn_connects_both_ends() {
        // The crossing needs both connects genuinely in flight at once. Loopback answers a SYN to
        // a port nobody is listening on with an instant RST, so on one CPU the first connect runs
        // to completion before the second is scheduled and no round can ever cross - a property of
        // the box, which this test cannot tell apart from a broken punch.
        if proxied() || std::thread::available_parallelism().map_or(true, |cpus| cpus.get() < 2) {
            return;
        }
        for _ in 0..256 {
            let (a, b) = free_loopback_pair().await;
            // Held for the whole crossing, because production always has one here and the design
            // rests on which of the two the kernel hands the connection to: the punch and the
            // peer's SYN share a four-tuple exactly, the listener only matches the address, and
            // the punch has to win that or every crossing would be swallowed as a plain accept.
            let listener = new_listener(a, true).await.unwrap();
            let to_b = tokio::spawn(socket_client::connect_tcp_local(b, Some(a), 3000));
            let to_a = tokio::spawn(socket_client::connect_tcp_local(a, Some(b), 3000));
            let (at_a, at_b) = tokio::join!(to_b, to_a);
            let (Ok(Ok(mut at_a)), Ok(Ok(mut at_b))) = (at_a, at_b) else {
                continue;
            };
            at_a.send_bytes(bytes::Bytes::from_static(b"punch"))
                .await
                .unwrap();
            let got = at_b.next_timeout(3000).await.unwrap().unwrap();
            assert_eq!(&got[..], b"punch", "both ends must share one connection");
            assert!(
                hbb_common::timeout(200, listener.accept()).await.is_err(),
                "the crossing must reach the punch, not be accepted as an inbound connection"
            );
            return;
        }
        panic!("no punch met the peer's SYN in 256 rounds on a machine that can cross them");
    }

    // The punch binds the address the listener already holds, so it has to go through the same
    // `connect_tcp_local` production uses - a punch built by hand here would still pass if
    // `new_socket` ever stopped setting the reuse flags, while every real punch failed to bind.
    // The peer's view of the source port is what proves the bind took: a fallback to an ephemeral
    // one would connect just as happily.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_punch_binds_the_address_the_listener_holds() {
        if proxied() {
            return;
        }
        // `free_loopback_pair` hands back ports it no longer holds, so another process can take
        // one in between; retry rather than fail for something the punch had no part in.
        for _ in 0..8 {
            let (local, peer_addr) = free_loopback_pair().await;
            let (Ok(listener), Ok(peer)) = (
                new_listener(local, true).await,
                new_listener(peer_addr, true).await,
            ) else {
                continue;
            };
            let punch = tokio::spawn(socket_client::connect_tcp_local(
                peer_addr,
                Some(local),
                1500,
            ));
            let (_peer_side, seen_as) = hbb_common::timeout(3000, peer.accept())
                .await
                .expect("the punch must reach the peer")
                .unwrap();
            assert_eq!(
                seen_as.port(),
                local.port(),
                "the punch must leave from the address the listener holds, not an ephemeral one"
            );
            // Held, not asserted and dropped: the coexistence below is only exercised while this
            // socket is still on the address, which is the state production spends its window in.
            let _punched = punch.await.unwrap().expect("the punch must connect");

            let dialed = tokio::spawn(tokio::net::TcpStream::connect(local));
            let accepted = hbb_common::timeout(3000, listener.accept()).await;
            assert!(
                matches!(accepted, Ok(Ok(_))),
                "the listener must still take connections while a punch shares its address: {accepted:?}"
            );
            assert!(dialed.await.unwrap().is_ok());
            return;
        }
        panic!("could not hold two free loopback addresses in 8 tries");
    }

    // The schedule on its own, against a paused clock: the window is CONNECT_TIMEOUT long, and
    // what these pin is where inside it the punches fall, which no socket could show.
    #[tokio::test(start_paused = true)]
    async fn the_punches_end_on_one_at_the_deadline() {
        use super::{punch_until, PUNCH_GRACE, PUNCH_INTERVAL, PUNCH_MAX_INTERVAL};
        use hbb_common::{anyhow::anyhow, config::CONNECT_TIMEOUT};
        use std::time::Duration;
        use tokio::time::Instant;

        let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let start = Instant::now();
        let until = start + Duration::from_millis(CONNECT_TIMEOUT);
        let mut punches = Vec::new();
        // A gateway that answers with RST: every punch fails the moment it is made.
        let met = punch_until::<(), _, _>(until, peer, |ms| {
            punches.push((Instant::now(), ms));
            async { Err(anyhow!("RST")) }
        })
        .await;
        assert!(met.is_none());
        assert_eq!(
            Instant::now(),
            until,
            "must return the moment the window closes, not a backoff later"
        );
        // Tokio rounds every sleep up to the next millisecond.
        let slack = Duration::from_millis(1);
        assert!(punches[0].0 - start <= Duration::from_secs_f32(PUNCH_INTERVAL) + slack);
        for pair in punches.windows(2) {
            assert!(
                pair[1].0 - pair[0].0 <= Duration::from_secs_f32(PUNCH_MAX_INTERVAL) + slack,
                "no gap in the window may exceed the backoff ceiling: {pair:?}"
            );
        }
        assert_eq!(
            *punches.last().unwrap(),
            (until, PUNCH_GRACE),
            "the window must end on a punch, given the whole grace"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_punch_in_flight_runs_the_grace_past_the_deadline_and_no_further() {
        use super::{punch_until, PUNCH_GRACE};
        use hbb_common::{anyhow::anyhow, config::CONNECT_TIMEOUT};
        use std::time::Duration;
        use tokio::time::Instant;

        let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let until = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT);
        let mut punches = 0;
        // A gateway that drops the SYN in silence: the punch sits in SYN_SENT for all it is given.
        let met = punch_until::<(), _, _>(until, peer, |ms| {
            punches += 1;
            async move {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                Err(anyhow!("timed out"))
            }
        })
        .await;
        assert!(met.is_none());
        assert_eq!(
            punches, 1,
            "a punch held in SYN_SENT is the only one the window needs"
        );
        assert_eq!(
            Instant::now(),
            until + Duration::from_millis(PUNCH_GRACE),
            "must return when the grace runs out, not a backoff later"
        );
    }
}
