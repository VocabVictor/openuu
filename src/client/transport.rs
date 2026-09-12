use super::*;

/// Closes an unadopted WebRTC offerer's pc on drop. Without an answer it stays in ICE `New`
/// forever, so its state handler never fires to self-remove it from `SESSIONS`; this covers the
/// early returns and cancelled races that would leak it. `into_inner` disarms on adoption.
pub(super) struct OffererGuard(Option<WebRTCStream>);

impl OffererGuard {
    pub(super) fn new(stream: WebRTCStream) -> Self {
        Self(Some(stream))
    }

    pub(super) fn stream(&self) -> Option<&WebRTCStream> {
        self.0.as_ref()
    }

    pub(super) fn into_inner(mut self) -> Option<WebRTCStream> {
        self.0.take()
    }
}

impl Drop for OffererGuard {
    fn drop(&mut self) {
        if let Some(stream) = self.0.take() {
            stream.close_detached();
        }
    }
}

/// Race WebRTC against the other transports, preferring P2P: `select_ok` would always pick the
/// relay, whose TCP connect beats ICE + DTLS + SCTP by an order of magnitude. An `is_p2p` result
/// wins outright; a relayed one — from either side, since `webrtc_fut` is a whole punch attempt
/// that can also end in a relay — is held for `window_ms` to give the other side a chance.
///
/// `others` must be non-empty (`select_ok` requires it).
pub(super) async fn race_transports_prefer_webrtc<'a, T: 'a>(
    webrtc_fut: BoxFuture<'a, ResultType<T>>,
    others: Vec<BoxFuture<'a, ResultType<T>>>,
    window_ms: u64,
    is_p2p: impl Fn(&T) -> bool,
) -> ResultType<T> {
    let mut webrtc_fut = Some(webrtc_fut);
    let mut others_fut = Some(select_ok(others));
    let mut held: Option<T> = None;
    let mut webrtc_err: Option<hbb_common::anyhow::Error> = None;
    let mut others_err: Option<hbb_common::anyhow::Error> = None;
    let window = tokio::time::sleep(Duration::from_millis(window_ms));
    tokio::pin!(window);
    let mut window_started = false;
    loop {
        tokio::select! {
            res = async {
                match webrtc_fut.as_mut() {
                    Some(fut) => fut.await,
                    None => std::future::pending().await,
                }
            }, if webrtc_fut.is_some() => {
                webrtc_fut = None;
                match res {
                    // A direct connection is the outcome this race exists to protect: commit it
                    // outright, and a held relay conn just drops.
                    Ok(conn) if is_p2p(&conn) || others_fut.is_none() => return Ok(conn),
                    // `webrtc_fut` is a whole punch attempt, not just the WebRTC connect, so it
                    // can end in a relay of its own. Committing that immediately would preempt a
                    // direct punch still in flight on the other branch — the exact inversion this
                    // function exists to prevent — so hold it on the same terms as any relay.
                    Ok(conn) => {
                        if held.is_none() {
                            held = Some(conn);
                            window.as_mut().reset(
                                Instant::now() + Duration::from_millis(window_ms),
                            );
                            window_started = true;
                        }
                    }
                    Err(e) => {
                        // Commit a held relay only when nothing direct is still racing; otherwise
                        // keep it and let the survivor (or the window) decide.
                        if others_fut.is_none() {
                            if let Some(conn) = held.take() {
                                return Ok(conn);
                            }
                        }
                        match others_err.take() {
                            Some(oe) => bail!("WebRTC failed: {}; fallback failed: {}", e, oe),
                            None if others_fut.is_none() => bail!("WebRTC failed: {}", e),
                            None => webrtc_err = Some(e),
                        }
                    }
                }
            }
            res = async {
                match others_fut.as_mut() {
                    Some(fut) => fut.await,
                    None => std::future::pending().await,
                }
            }, if others_fut.is_some() => {
                others_fut = None;
                match res {
                    Ok((conn, unfinished)) => {
                        if is_p2p(&conn) {
                            return Ok(conn);
                        }
                        // Relayed: commit now only if nothing direct can still arrive. If a
                        // direct attempt is still in flight (here or in `unfinished`), hold it
                        // and keep racing for the preference window instead of discarding them.
                        if webrtc_fut.is_none() && unfinished.is_empty() {
                            return Ok(conn);
                        }
                        if held.is_none() {
                            held = Some(conn);
                            window.as_mut().reset(
                                Instant::now() + Duration::from_millis(window_ms),
                            );
                            window_started = true;
                        }
                        if !unfinished.is_empty() {
                            others_fut = Some(select_ok(unfinished));
                        }
                    }
                    Err(e) => match webrtc_err.take() {
                        // Nothing more can win, but a parked relay is still a valid outcome — take
                        // it before failing the connection.
                        Some(we) => match held.take() {
                            Some(conn) => return Ok(conn),
                            None => bail!("WebRTC failed: {}; fallback failed: {}", we, e),
                        },
                        None if webrtc_fut.is_none() => match held.take() {
                            Some(conn) => return Ok(conn),
                            None => return Err(e),
                        },
                        None => others_err = Some(e),
                    },
                }
            }
            _ = &mut window, if window_started => {
                if let Some(conn) = held.take() {
                    return Ok(conn);
                }
                window_started = false;
            }
        }
    }
}

// A peer decides how many ICE candidates it sends, and the rendezvous route that carries them is
// reachable without a prior punch, so these sites would otherwise let someone else set how much
// this machine writes to its log file. One line a minute each, carrying the suppressed count.
use hbb_common::log_throttle::LogThrottle;
pub(super) const ICE_LOG_INTERVAL: Duration = Duration::from_secs(60);
pub(super) static REJECTED_ICE_LOG: LogThrottle = LogThrottle::new(ICE_LOG_INTERVAL);
pub(super) static UDP_UAT_ERR_LOG: LogThrottle = LogThrottle::new(ICE_LOG_INTERVAL);
pub(super) static UNEXPECTED_ICE_LOG: LogThrottle = LogThrottle::new(ICE_LOG_INTERVAL);
pub(super) static PENDING_ICE_FULL_LOG: LogThrottle = LogThrottle::new(ICE_LOG_INTERVAL);

pub(super) fn request_allows_tcp_punch(webrtc_sdp_offer: &str) -> bool {
    // WebRTC trickle ICE retains the rendezvous socket as its signaling bridge. Only a request
    // without an offer may close that socket and reuse its local address for TCP punching.
    webrtc_sdp_offer.is_empty()
}

/// TCP punch is a user option like the other direct transports, but it is also the backstop:
/// with every direct transport switched off there would be nothing left to punch with, so it
/// runs regardless. Only the switches decide that — a transport that is enabled but fails to
/// materialize (no public v6 address, offerer setup error) leaves this alone, because the
/// relay fallback already covers a round that ends up with no usable direct transport.
pub(super) fn tcp_punch_allowed() -> bool {
    crate::get_tcp_punch_enabled()
        || !(crate::get_udp_punch_enabled()
            || crate::get_ipv6_punch_enabled()
            || crate::get_webrtc_enabled())
}
