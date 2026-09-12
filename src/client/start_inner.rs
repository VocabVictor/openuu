use super::*;

impl Client {
    pub(super) async fn _start_inner(
        peer: String,
        key: String,
        token: String,
        conn_type: ConnType,
        interface: impl Interface,
        mut udp: (Option<Arc<UdpSocket>>, Option<Arc<Mutex<u16>>>),
        stop_udp_tx: Option<oneshot::Sender<()>>,
        mut ipv6: Option<(Arc<UdpSocket>, bytes::Bytes)>,
        webrtc_offerer: Option<WebRTCStream>,
        mut rendezvous_server: String,
        servers: Vec<String>,
        contained: bool,
    ) -> ResultType<(
        (
            Stream,
            bool,
            Option<Vec<u8>>,
            Option<KcpStream>,
            &'static str,
        ),
        (i32, String),
        bool,
    )> {
        // Wrap the offerer so any early return below (?/bail) or cancellation of this future by
        // the outer select_ok closes its pc instead of leaking it in SESSIONS. Disarmed via
        // into_inner() once the stream is adopted into a connection attempt.
        let mut webrtc_offerer = webrtc_offerer.map(OffererGuard::new);
        let mut start = Instant::now();
        let mut socket = connect_tcp(&*rendezvous_server, CONNECT_TIMEOUT).await;
        debug_assert!(!servers.contains(&rendezvous_server));
        let rtt = start.elapsed();
        log::debug!("TCP connection establishment time used: {:?}", rtt);
        if socket.is_err() && !servers.is_empty() {
            log::info!("try the other servers: {:?}", servers);
            for server in servers {
                let server = check_port(server, RENDEZVOUS_PORT);
                socket = connect_tcp(&*server, CONNECT_TIMEOUT).await;
                if socket.is_ok() {
                    rendezvous_server = server;
                    break;
                }
            }
            crate::refresh_rendezvous_server();
        } else if !contained {
            crate::refresh_rendezvous_server();
        }
        log::info!("rendezvous server: {}", rendezvous_server);
        let mut socket = socket?;
        let my_addr = socket.local_addr();
        let mut signed_id_pk = Vec::new();
        let mut relay_server = "".to_owned();
        let mut peer_addr = Config::get_any_listen_addr(true);
        let mut peer_nat_type = NatType::UNKNOWN_NAT;
        let my_nat_type = crate::get_nat_type(100).await;
        let mut is_local = false;
        let mut feedback = 0;
        use hbb_common::protobuf::Enum;
        let nat_type = if interface.is_force_relay() {
            NatType::SYMMETRIC
        } else {
            NatType::from_i32(my_nat_type).unwrap_or(NatType::UNKNOWN_NAT)
        };

        let switch_code = interface.get_switch_code();
        if !key.is_empty() && (!token.is_empty() || !switch_code.is_empty()) {
            secure_tcp(&mut socket, &key)
                .await
                .map_err(|e| anyhow!("Failed to secure tcp: {}", e))?;
        } else if let Some(udp) = udp.1.as_ref() {
            let tm = Instant::now();
            // rtt is the TCP connect time. When it is too short to be a real WAN round trip it
            // says nothing about the UDP path (a TUN VPN or the LAN gateway answered the
            // handshake, not the server), so fall back to the flat grace; otherwise trust it.
            let udp_nat_wait = if rtt < Self::TCP_RTT_PLAUSIBLE_MIN {
                Self::UDP_NAT_TEST_GRACE
            } else {
                rtt / 2
            };
            loop {
                let port = *udp.lock().unwrap();
                if port > 0 {
                    break;
                }
                if tm.elapsed() > udp_nat_wait {
                    break;
                }
                hbb_common::sleep(0.001).await;
            }
        }
        // Stop UDP NAT test task if still running
        stop_udp_tx.map(|tx| tx.send(()));
        let mut msg_out = RendezvousMessage::new();
        let mut ipv6 = ipv6
            .take()
            .map(|(socket, addr)| (Some(socket), Some(addr)))
            .unwrap_or((None, None));
        let udp_nat_port = udp.1.map(|x| *x.lock().unwrap()).unwrap_or(0);
        let webrtc_sdp_offer = webrtc_offerer
            .as_ref()
            .and_then(|g| g.stream())
            .map(|stream| stream.local_endpoint().to_owned())
            .unwrap_or_default();
        let allow_tcp_punch = tcp_punch_allowed() && request_allows_tcp_punch(&webrtc_sdp_offer);
        // Every direct transport this round carries, not one of them: a round can carry several
        // at once (a NAT port and an offer and a v6 address), and since the TCP punch became a
        // switch it can carry none — a single name had to misreport both. `relay` is not a punch,
        // it is what a round with nothing to punch with can still end as.
        let mut transports = Vec::new();
        if udp_nat_port > 0 {
            transports.push("UDP");
        }
        if allow_tcp_punch {
            transports.push("TCP");
        }
        if ipv6.1.is_some() {
            transports.push("IPv6");
        }
        if !webrtc_sdp_offer.is_empty() {
            transports.push("WebRTC");
        }
        let punch_type = if transports.is_empty() {
            "Relay".to_owned()
        } else {
            transports.join("+")
        };
        msg_out.set_punch_hole_request(PunchHoleRequest {
            id: peer.to_owned(),
            token: token.to_owned(),
            nat_type: nat_type.into(),
            licence_key: key.to_owned(),
            conn_type: conn_type.into(),
            version: crate::VERSION.to_owned(),
            udp_port: udp_nat_port as _,
            force_relay: interface.is_force_relay(),
            socket_addr_v6: ipv6.1.unwrap_or_default(),
            switch_code,
            // The offer's envelope itself declares its ICE policy (`ice_policy: "all"` under
            // pure ws), telling the controlled side its answer may gather every candidate
            // type despite force_relay instead of requiring TURN.
            webrtc_sdp_offer,
            ..Default::default()
        });
        let webrtc_session_key = webrtc_offerer
            .as_ref()
            .and_then(|guard| guard.stream())
            .map(|stream| stream.session_key().to_owned())
            .unwrap_or_default();
        let mut webrtc_sdp_answer = String::new();
        let mut pending_webrtc_ice = Vec::<String>::new();
        'punch_attempts: for i in 1..=3 {
            log::info!(
                "#{} {} punch attempt with {}, id: {}",
                i,
                punch_type,
                my_addr,
                peer
            );
            socket.send(&msg_out).await?;
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
                            peer_nat_type = ph.nat_type();
                            is_local = ph.is_local();
                            signed_id_pk = ph.pk.into();
                            relay_server = ph.relay_server;
                            peer_addr = AddrMangle::decode(&ph.socket_addr);
                            feedback = ph.feedback;
                            webrtc_sdp_answer = ph.webrtc_sdp_answer;
                            let s = udp.0.take();
                            if udp_nat_port > 0 && ph.is_udp && s.is_some() {
                                if let Some(s) = s {
                                    allow_err!(s.connect(peer_addr).await);
                                    udp.0 = Some(s);
                                }
                            }
                            let s = ipv6.0.take();
                            if !ph.socket_addr_v6.is_empty() && s.is_some() {
                                let addr = AddrMangle::decode(&ph.socket_addr_v6);
                                if addr.port() > 0 {
                                    if let Some(s) = s {
                                        allow_err!(s.connect(addr).await);
                                        ipv6.0 = Some(s);
                                    }
                                }
                            }
                            log::info!("{} Hole Punched {} = {}", punch_type, peer, peer_addr);
                            break 'punch_attempts;
                        }
                    }
                    Some(rendezvous_message::Union::RelayResponse(rr)) => {
                        log::info!(
                            "relay requested from peer, time used: {:?}, relay_server: {}",
                            start.elapsed(),
                            rr.relay_server
                        );
                        start = Instant::now();
                        let mut connect_futures = Vec::new();
                        if let Some(s) = ipv6.0 {
                            let addr = AddrMangle::decode(&rr.socket_addr_v6);
                            if addr.port() > 0 {
                                if s.connect(addr).await.is_ok() {
                                    connect_futures.push(
                                        async move {
                                            let (conn, kcp, typ) =
                                                udp_nat_connect(s, "IPv6", CONNECT_TIMEOUT).await?;
                                            Ok((conn, kcp, typ, true))
                                        }
                                        .boxed(),
                                    );
                                }
                            }
                        }
                        signed_id_pk = rr.pk().into();
                        let mut webrtc_bridge_stop = None;
                        let mut webrtc_for_connect = None;
                        if !rr.webrtc_sdp_answer.is_empty() {
                            if let Some(guard) = webrtc_offerer.take() {
                                // Run the awaited setup on the guard's borrowed stream so a
                                // cancellation during these awaits still closes the pc via the
                                // guard's drop; take ownership only at the synchronous handoff.
                                let setup_ok = if let Some(webrtc) = guard.stream() {
                                    if let Err(err) =
                                        webrtc.set_remote_endpoint(&rr.webrtc_sdp_answer).await
                                    {
                                        log::warn!("failed to set WebRTC relay answer: {}", err);
                                        false
                                    } else {
                                        for candidate in pending_webrtc_ice.drain(..) {
                                            if let Err(err) =
                                                webrtc.add_remote_ice_candidate(&candidate).await
                                            {
                                                log::warn!(
                                                    "failed to add buffered WebRTC ICE candidate: {}",
                                                    err
                                                );
                                            }
                                        }
                                        true
                                    }
                                } else {
                                    false
                                };
                                if let Some(webrtc) = setup_ok.then(|| guard.into_inner()).flatten()
                                {
                                    let session_key = webrtc.session_key().to_owned();
                                    let local_ice_rx = webrtc.take_local_ice_rx();
                                    webrtc_bridge_stop = Some(Self::spawn_webrtc_ice_bridge(
                                        socket,
                                        local_ice_rx,
                                        webrtc.clone(),
                                        peer.clone(),
                                        session_key,
                                    ));
                                    webrtc_for_connect = Some(webrtc);
                                }
                                // If setup failed, `guard` drops here and closes the pc.
                            }
                        }
                        // An offerer not adopted into the relay race (empty answer) is closed by
                        // the guard's drop here.
                        drop(webrtc_offerer.take());
                        // Keep relay_server for a WebRTC secure-failure fallback: request_relay
                        // coordinates a FRESH uuid via the rendezvous server, so it works even if
                        // the raced create_relay already consumed the original uuid pairing.
                        let relay_server_rr = rr.relay_server.clone();
                        let fut = Self::create_relay(
                            &peer,
                            rr.uuid,
                            rr.relay_server,
                            &key,
                            conn_type,
                            my_addr.is_ipv4(),
                        );
                        connect_futures.push(
                            async move {
                                let conn = fut.await?;
                                Ok((
                                    conn,
                                    None,
                                    if use_ws() { "WebSocket" } else { "Relay" },
                                    false,
                                ))
                            }
                            .boxed(),
                        );
                        // Keep the adopted offerer in a guard that stays armed across the race AND
                        // secure_connection, so cancellation of this future by the outer race (or a
                        // secure-handshake failure) closes the pc instead of leaking it. It is
                        // disarmed only once WebRTC is confirmed the winning, secured transport.
                        let mut webrtc_guard = None;
                        let race_result = if let Some(webrtc) = webrtc_for_connect {
                            webrtc_guard = Some(OffererGuard::new(webrtc.clone()));
                            let mut raced = webrtc;
                            let webrtc_fut = async move {
                                raced.wait_connected(CONNECT_TIMEOUT).await?;
                                // Resolve relayed-ness here, not from the label: WebRTC is only a
                                // P2P path when ICE nominated a non-TURN pair, and the race has to
                                // know which it got. Committing a TURN pair as if it were direct
                                // cancels a genuine direct attempt still in flight — the same
                                // inversion the preference window exists to prevent, one level up.
                                let relayed = raced.is_relayed().await.unwrap_or(true);
                                Ok((Stream::WebRTC(raced), None, "WebRTC", !relayed))
                            }
                            .boxed();
                            if interface.is_policy_relay() {
                                // Relay-only WebRTC can use only TURN, so it has no P2P advantage
                                // over the RustDesk relay. Take the first successful relay instead
                                // of delaying an already-ready result for the preference window.
                                // Policy, not force_relay: under ws the offer is full ICE and a
                                // direct path is exactly what the preference window exists for.
                                connect_futures.push(webrtc_fut);
                                select_ok(connect_futures).await.map(|r| r.0)
                            } else {
                                // The peer answered WebRTC: prefer P2P. The relay result is held
                                // for the preference window so WebRTC can win even though a relay
                                // TCP connect completes much faster than ICE + DTLS setup.
                                race_transports_prefer_webrtc(
                                    webrtc_fut,
                                    connect_futures,
                                    Self::WEBRTC_PREFER_WINDOW_MS,
                                    |result| result.3,
                                )
                                .await
                            }
                        } else {
                            // Run all connection attempts concurrently, take the first success.
                            select_ok(connect_futures).await.map(|r| r.0)
                        };
                        if let Some(stop) = webrtc_bridge_stop {
                            let _ = stop.send(());
                        }
                        // The ? / secure_connection failures below return early; webrtc_guard stays
                        // in scope and closes the offerer on any such exit (loss, error, cancellation).
                        let (mut conn, kcp, mut typ, mut direct) = race_result?;
                        feedback = rr.feedback;
                        log::info!("{:?} used to establish {typ} connection", start.elapsed());
                        let pk = match Self::secure_connection(
                            &peer,
                            signed_id_pk.clone(),
                            &key,
                            &mut conn,
                        )
                        .await
                        {
                            Ok(pk) => pk,
                            Err(e) if typ == "WebRTC" => {
                                // WebRTC won the race but identity/DTLS binding failed. Fall back
                                // to a freshly-coordinated relay (request_relay negotiates a new
                                // uuid, immune to the raced create_relay having consumed the
                                // original pairing) so a bad WebRTC handshake does not kill the
                                // whole session when relay is available.
                                log::warn!(
                                    "WebRTC secure handshake failed ({}), falling back to relay",
                                    e
                                );
                                drop(webrtc_guard.take());
                                let mut relay_conn = Self::request_relay(
                                    &peer,
                                    relay_server_rr,
                                    &rendezvous_server,
                                    !signed_id_pk.is_empty(),
                                    &key,
                                    &token,
                                    conn_type,
                                    &interface.get_switch_code(),
                                )
                                .await
                                .map_err(|relay_e| {
                                    anyhow!(
                                        "WebRTC secure failed ({}); relay fallback also failed: {}",
                                        e,
                                        relay_e
                                    )
                                })?;
                                let pk = Self::secure_connection(
                                    &peer,
                                    signed_id_pk,
                                    &key,
                                    &mut relay_conn,
                                )
                                .await?;
                                conn = relay_conn;
                                typ = if use_ws() { "WebSocket" } else { "Relay" };
                                // The transport is now a relay: the WebRTC win it replaced must
                                // not carry its direct flag into the return, or the relay is
                                // reported P2P and the outer race treats it as one.
                                direct = false;
                                pk
                            }
                            Err(e) => return Err(e),
                        };
                        // `direct` came from the winning future, which resolved it while the pc was
                        // definitely alive — the race needed it to pick a winner at all.
                        // Secured and WebRTC won: disarm so the returned conn keeps the pc alive.
                        if typ == "WebRTC" {
                            if let Some(guard) = webrtc_guard.take() {
                                let _ = guard.into_inner();
                            }
                        }
                        return Ok((
                            (conn, direct, pk, kcp, typ),
                            (feedback, rendezvous_server),
                            false,
                        ));
                    }
                    Some(rendezvous_message::Union::IceCandidate(ice)) => {
                        if Self::is_expected_webrtc_ice_candidate(&ice, &webrtc_session_key) {
                            // Evict the oldest, not the newest. Candidates arrive in gathering
                            // order — host first, then srflx, then relay — so dropping arrivals
                            // would discard exactly the ones that traverse NAT and keep the
                            // host ones that only work on a shared LAN.
                            if pending_webrtc_ice.len() >= Self::MAX_PENDING_WEBRTC_ICE {
                                if let Some(n) = PENDING_ICE_FULL_LOG.due() {
                                    log::warn!(
                                        "WebRTC ICE pending buffer full ({}), evicted {} oldest",
                                        Self::MAX_PENDING_WEBRTC_ICE,
                                        n
                                    );
                                }
                                pending_webrtc_ice.remove(0);
                            }
                            pending_webrtc_ice.push(ice.candidate);
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
        let mut webrtc_bridge_stop = None;
        let mut webrtc_for_connect = None;
        if !webrtc_sdp_answer.is_empty() {
            if let Some(guard) = webrtc_offerer.take() {
                // Run the awaited setup on the guard's borrowed stream so a cancellation during
                // these awaits still closes the pc via the guard's drop; take ownership only at
                // the synchronous handoff below.
                let setup_ok = if let Some(webrtc) = guard.stream() {
                    if let Err(err) = webrtc.set_remote_endpoint(&webrtc_sdp_answer).await {
                        log::warn!("failed to set WebRTC answer: {}", err);
                        false
                    } else {
                        for candidate in pending_webrtc_ice.drain(..) {
                            if let Err(err) = webrtc.add_remote_ice_candidate(&candidate).await {
                                log::warn!("failed to add buffered WebRTC ICE candidate: {}", err);
                            }
                        }
                        true
                    }
                } else {
                    false
                };
                if let Some(webrtc) = setup_ok.then(|| guard.into_inner()).flatten() {
                    let session_key = webrtc.session_key().to_owned();
                    let local_ice_rx = webrtc.take_local_ice_rx();
                    webrtc_bridge_stop = Some(Self::spawn_webrtc_ice_bridge(
                        socket,
                        local_ice_rx,
                        webrtc.clone(),
                        peer.clone(),
                        session_key,
                    ));
                    webrtc_for_connect = Some(webrtc);
                } else {
                    // setup failed (guard dropped -> pc closed) or no stream: release the socket.
                    drop(socket);
                }
            } else {
                drop(socket);
            }
        } else {
            drop(socket);
        }
        // An offerer never adopted into a connection attempt (e.g. the peer returned no WebRTC
        // answer) is closed by the guard's drop here, so its pc does not linger in SESSIONS.
        drop(webrtc_offerer.take());
        if peer_addr.port() == 0 {
            // Bailing before connect(): an offerer already adopted into webrtc_for_connect was
            // disarmed out of its guard, so close it (and stop its bridge) explicitly here.
            if let Some(webrtc) = webrtc_for_connect.take() {
                webrtc.close_detached();
            }
            if let Some(stop) = webrtc_bridge_stop.take() {
                let _ = stop.send(());
            }
            bail!("Failed to connect via rendezvous server");
        }
        let time_used = start.elapsed().as_millis() as u64;
        log::info!(
            "{} ms used to {} punch hole, relay_server: {}, {}",
            time_used,
            punch_type,
            relay_server,
            if is_local {
                "is_local: true".to_owned()
            } else {
                format!("nat_type: {:?}", peer_nat_type)
            }
        );
        Ok((
            Self::connect(
                my_addr,
                peer_addr,
                &peer,
                signed_id_pk,
                &relay_server,
                &rendezvous_server,
                time_used,
                peer_nat_type,
                my_nat_type,
                is_local,
                &key,
                &token,
                conn_type,
                interface,
                udp.0,
                ipv6.0,
                webrtc_for_connect,
                webrtc_bridge_stop,
                allow_tcp_punch,
                &punch_type,
            )
            .await?,
            (feedback, rendezvous_server),
            true,
        ))
    }
}
