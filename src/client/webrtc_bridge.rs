use super::*;

impl Client {
    pub(super) fn is_expected_webrtc_ice_candidate(ice: &IceCandidate, session_key: &str) -> bool {
        !session_key.is_empty() && ice.session_key == session_key && !ice.candidate.is_empty()
    }

    /// Whether to build a WebRTC offerer for this connection.
    ///
    /// A SOCKS proxy rules it out: ICE binds its own UDP sockets and speaks STUN directly, past
    /// the proxy and with the real IP. Relay-by-policy without TURN rules it out too, since
    /// Relay-only ICE can then gather nothing. WebSocket does not: it tunnels only the signaling
    /// and relay legs, so the offer keeps full ICE and direct is exactly what it is there for.
    pub(super) fn should_create_webrtc_offerer(interface: &impl Interface) -> bool {
        if !crate::get_webrtc_enabled() {
            return false;
        }
        if Config::is_proxy() {
            return false;
        }
        if interface.is_policy_relay() && !WebRTCStream::has_turn_server() {
            return false;
        }
        true
    }

    /// Max ICE candidates buffered during the punch window before the answer is applied.
    /// Bounds memory if a misbehaving rendezvous floods candidates. On overflow the oldest is
    /// evicted: gathering order is host, then srflx, then relay, so the newest arrivals are the
    /// ones that traverse NAT.
    pub(super) const MAX_PENDING_WEBRTC_ICE: usize = 64;

    /// Prefer-P2P window: how long a WebRTC attempt outranks an already-established relay
    /// result, and the floor for a punch-path WebRTC attempt whose race timeout is tuned for a
    /// raw TCP SYN. Long enough for candidate trickle + ICE checks + DTLS on high-latency
    /// links; short enough that UDP-blocked networks settle on relay without a noticeable wait.
    pub(super) const WEBRTC_PREFER_WINDOW_MS: u64 = 2500;

    /// UDP-NAT-test wait when the TCP clock is implausible (see TCP_RTT_PLAUSIBLE_MIN). The
    /// normal bound is `rtt / 2`: the test has been running since before the TCP connect, so on
    /// a network where TCP RTT ~ UDP RTT its response has already landed. A transparent TCP
    /// proxy — a TUN-mode VPN on the host, or a redirect-mode proxy on the LAN gateway (soft
    /// router), which fakes the handshake for every device behind it — breaks that by answering
    /// in ~3ms while the real UDP round trip is hundreds of ms: the window collapsed to ~1.5ms,
    /// udp_port stayed 0, and UDP punch never ran on such networks. The wait still exits the instant the port
    /// arrives, so a genuinely nearby server (LAN hbbs) pays nothing; only UDP-dead networks
    /// wait out the full grace, and only on this round — the pure-TCP fallback round never
    /// waits. Sized as a ceiling on real-world rendezvous RTTs plus one 20ms retransmit
    /// (intercontinental ~300ms); paths slower than that lose UDP punch under a proxy, which
    /// is today's behavior, not a regression.
    pub(super) const UDP_NAT_TEST_GRACE: Duration = Duration::from_millis(400);

    /// Below this, the measured TCP connect time is not a believable WAN round trip — it was
    /// answered inside the LAN (host TUN proxy, gateway transparent proxy, or a genuinely local
    /// server) — and must not be used to size the UDP window.
    /// Generous on purpose: over-triggering costs nothing (the wait exits on arrival, and a
    /// UDP-dead round loses to the racing fallback anyway), while a tight bound would let a
    /// busy proxy's occasional ~80ms handshake slip through and silently drop UDP punch.
    pub(super) const TCP_RTT_PLAUSIBLE_MIN: Duration = Duration::from_millis(100);

    /// Delay before re-sending an ICE candidate over the rendezvous route once. The hop to the
    /// peer can be UDP (the controlled side's mediator channel), so a candidate can be lost in
    /// flight; the remote ICE agent dedups repeats, so the second copy is free.
    pub(super) const WEBRTC_ICE_RESEND_DELAY: Duration = Duration::from_millis(400);

    /// Bridge local ICE candidates to the peer over the punch socket, and feed the peer's back
    /// into the pc.
    ///
    /// Never reconnect this socket: its address *is* the return route (the server mangles it into
    /// `PunchHole.socket_addr` and resolves the echo through `tcp_punch`), so a new address is one
    /// nothing points at. Once it dies, both directions are dead — abandon WebRTC, do not retry.
    pub(super) fn spawn_webrtc_ice_bridge(
        mut socket: Stream,
        mut local_ice_rx: Option<UnboundedReceiver<String>>,
        webrtc: WebRTCStream,
        peer: String,
        session_key: String,
    ) -> oneshot::Sender<()> {
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let mut pending_resend: Vec<(Instant, RendezvousMessage)> = Vec::new();
            loop {
                match stop_rx.try_recv() {
                    Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => break,
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
                }

                if let Some(rx) = local_ice_rx.as_mut() {
                    loop {
                        match rx.try_recv() {
                            Ok(candidate) => {
                                let mut msg = RendezvousMessage::new();
                                msg.set_ice_candidate(IceCandidate {
                                    id: peer.clone(),
                                    session_key: session_key.clone(),
                                    candidate,
                                    ..Default::default()
                                });
                                // Bound the send so a stalled rendezvous socket cannot block the
                                // bridge past its stop signal.
                                match timeout(3000, socket.send(&msg)).await {
                                    Ok(Ok(())) => {}
                                    Ok(Err(err)) => {
                                        log::warn!("failed to send WebRTC ICE candidate: {}", err);
                                        return;
                                    }
                                    Err(_) => {
                                        log::warn!("WebRTC ICE candidate send timed out");
                                        return;
                                    }
                                }
                                pending_resend.push((Instant::now(), msg));
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                local_ice_rx = None;
                                break;
                            }
                        }
                    }
                }

                // Re-send each candidate once after a short delay: the rendezvous hop to the peer
                // can be UDP, so the first copy may be lost; the remote ICE agent dedups repeats.
                let mut i = 0;
                while i < pending_resend.len() {
                    if pending_resend[i].0.elapsed() >= Self::WEBRTC_ICE_RESEND_DELAY {
                        let (_, msg) = pending_resend.swap_remove(i);
                        match timeout(3000, socket.send(&msg)).await {
                            Ok(Ok(())) => {}
                            Ok(Err(err)) => {
                                log::warn!("failed to re-send WebRTC ICE candidate: {}", err);
                                return;
                            }
                            Err(_) => {
                                log::warn!("WebRTC ICE candidate re-send timed out");
                                return;
                            }
                        }
                    } else {
                        i += 1;
                    }
                }

                // Read one incoming message, blocking up to the poll window. Distinguish a real
                // timeout (keep looping to drain outbound candidates) from stream EOF/error: the
                // rendezvous server closes the punch connection after the response, and an
                // unrecognized-message server closes it on the first candidate we send. Without
                // this, the collapsed None-on-EOF made the loop hot-spin a core until stop.
                match timeout(100, socket.next()).await {
                    Err(_) => {}
                    Ok(None) => break,
                    Ok(Some(Ok(bytes))) => {
                        if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                            if let Some(rendezvous_message::Union::IceCandidate(ice)) = msg_in.union
                            {
                                if Self::is_expected_webrtc_ice_candidate(&ice, &session_key) {
                                    if let Err(err) =
                                        webrtc.add_remote_ice_candidate(&ice.candidate).await
                                    {
                                        if let Some(n) = REJECTED_ICE_LOG.due() {
                                            log::warn!(
                                                "failed to add {} WebRTC ICE candidate(s), last: {}",
                                                n,
                                                err
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Some(Err(err))) => {
                        log::debug!("WebRTC ICE bridge socket read ended: {}", err);
                        break;
                    }
                }
            }
        });
        stop_tx
    }
}
