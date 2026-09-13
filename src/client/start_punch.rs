use super::*;

/// What a punch round leaves behind for the direct connect that follows it.
pub(super) struct PunchState {
    pub(super) peer_nat_type: NatType,
    pub(super) is_local: bool,
    pub(super) signed_id_pk: Vec<u8>,
    pub(super) relay_server: String,
    pub(super) peer_addr: SocketAddr,
    pub(super) feedback: i32,
    pub(super) webrtc_sdp_answer: String,
    pub(super) pending_webrtc_ice: Vec<String>,
}

pub(super) enum PunchOutcome<'a, I: Interface> {
    /// The rendezvous server answered the punch (or never did); connect directly next.
    Punched { socket: Stream, ctx: StartCtx<'a, I> },
    /// The server sent a RelayResponse and the connection was made from it.
    Connected(StartResult),
}

impl Client {
    /// Send the punch request up to three times and handle what the rendezvous server sends
    /// back until it either reports the peer address or turns the round into a relay.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn punch_hole_attempts<'a, I: Interface>(
        mut socket: Stream,
        msg_out: &RendezvousMessage,
        punch_type: &str,
        udp_nat_port: u16,
        udp_socket: &mut Option<Arc<UdpSocket>>,
        ipv6_socket: &mut Option<Arc<UdpSocket>>,
        webrtc_offerer: &mut Option<OffererGuard>,
        webrtc_session_key: &str,
        state: &mut PunchState,
        ctx: StartCtx<'a, I>,
    ) -> ResultType<PunchOutcome<'a, I>> {
    for i in 1..=3 {
        log::info!(
            "#{} {} punch attempt with {}, id: {}",
            i,
            punch_type,
            ctx.my_addr,
            ctx.peer
        );
        socket.send(msg_out).await?;
        // below timeout should not bigger than hbbs's connection timeout.
        let attempt_deadline = Instant::now() + Duration::from_millis((i * 3000) as u64);
        loop {
            let remaining = attempt_deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let timeout_ms = remaining
                .as_millis()
                .clamp(1, u64::MAX as u128) as u64;
            let Some(msg_in) =
                crate::get_next_nonkeyexchange_msg(&mut socket, Some(timeout_ms)).await
            else {
                break;
            };
            match msg_in.union {
                Some(rendezvous_message::Union::PunchHoleResponse(ph)) => {
                    if ph.socket_addr.is_empty() {
                        if !ph.other_failure.is_empty() {
                            bail!(ph.other_failure);
                        }
                        match ph.failure.enum_value() {
                            Ok(punch_hole_response::Failure::ID_NOT_EXIST) => {
                                bail!("ID does not exist");
                            }
                            Ok(punch_hole_response::Failure::OFFLINE) => {
                                bail!("Remote desktop is offline");
                            }
                            Ok(punch_hole_response::Failure::LICENSE_MISMATCH) => {
                                bail!("Key mismatch");
                            }
                            Ok(punch_hole_response::Failure::LICENSE_OVERUSE) => {
                                bail!("Key overuse");
                            }
                            _ => bail!("other punch hole failure"),
                        }
                    } else {
                        state.peer_nat_type = ph.nat_type();
                        state.is_local = ph.is_local();
                        state.signed_id_pk = ph.pk.into();
                        state.relay_server = ph.relay_server;
                        state.peer_addr = AddrMangle::decode(&ph.socket_addr);
                        state.feedback = ph.feedback;
                        state.webrtc_sdp_answer = ph.webrtc_sdp_answer;
                        let s = udp_socket.take();
                        if udp_nat_port > 0 && ph.is_udp && s.is_some() {
                            if let Some(s) = s {
                                allow_err!(s.connect(state.peer_addr).await);
                                *udp_socket = Some(s);
                            }
                        }
                        let s = ipv6_socket.take();
                        if !ph.socket_addr_v6.is_empty() && s.is_some() {
                            let addr = AddrMangle::decode(&ph.socket_addr_v6);
                            if addr.port() > 0 {
                                if let Some(s) = s {
                                    allow_err!(s.connect(addr).await);
                                    *ipv6_socket = Some(s);
                                }
                            }
                        }
                        log::info!("{} Hole Punched {} = {}", punch_type, ctx.peer, state.peer_addr);
                        return Ok(PunchOutcome::Punched { socket, ctx });
                    }
                }
                Some(rendezvous_message::Union::RelayResponse(rr)) => {
                    return Self::connect_on_relay_response(
                        rr,
                        socket,
                        ipv6_socket.take(),
                        webrtc_offerer.take(),
                        std::mem::take(&mut state.pending_webrtc_ice),
                        ctx,
                    )
                    .await
                    .map(PunchOutcome::Connected);
                }
                Some(rendezvous_message::Union::IceCandidate(ice)) => {
                    if Self::is_expected_webrtc_ice_candidate(&ice, webrtc_session_key) {
                        // Evict the oldest, not the newest. Candidates arrive in gathering
                        // order — host first, then srflx, then relay — so dropping arrivals
                        // would discard exactly the ones that traverse NAT and keep the
                        // host ones that only work on a shared LAN.
                        if state.pending_webrtc_ice.len() >= Self::MAX_PENDING_WEBRTC_ICE {
                            if let Some(n) = PENDING_ICE_FULL_LOG.due() {
                                log::warn!(
                                    "WebRTC ICE pending buffer full ({}), evicted {} oldest",
                                    Self::MAX_PENDING_WEBRTC_ICE,
                                    n
                                );
                            }
                            state.pending_webrtc_ice.remove(0);
                        }
                        state.pending_webrtc_ice.push(ice.candidate);
                    } else if let Some(n) = UNEXPECTED_ICE_LOG.due() {
                        log::debug!(
                            "dropped {} ICE candidate(s) for unexpected WebRTC session key, last: {}",
                            n,
                            ice.session_key,
                        );
                    }
                }
                _ => {
                    log::error!("Unexpected protobuf msg received: {:?}", msg_in);
                }
            }
        }
    }
        Ok(PunchOutcome::Punched { socket, ctx })
    }
}
