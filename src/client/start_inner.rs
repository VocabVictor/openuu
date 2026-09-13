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
        let start = Instant::now();
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
        let mut state = PunchState {
            peer_nat_type: NatType::UNKNOWN_NAT,
            is_local: false,
            signed_id_pk: Vec::new(),
            relay_server: "".to_owned(),
            peer_addr: Config::get_any_listen_addr(true),
            feedback: 0,
            webrtc_sdp_answer: String::new(),
            pending_webrtc_ice: Vec::new(),
        };
        let my_nat_type = crate::get_nat_type(100).await;
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
        let ctx = StartCtx {
            peer: &peer,
            key: &key,
            token: &token,
            conn_type,
            interface,
            rendezvous_server: &rendezvous_server,
            my_addr,
            start,
        };
        let (socket, ctx) = match Self::punch_hole_attempts(
            socket,
            &msg_out,
            &punch_type,
            udp_nat_port,
            &mut udp.0,
            &mut ipv6.0,
            &mut webrtc_offerer,
            &webrtc_session_key,
            &mut state,
            ctx,
        )
        .await?
        {
            PunchOutcome::Connected(result) => return Ok(result),
            PunchOutcome::Punched { socket, ctx } => (socket, ctx),
        };
        let StartCtx { interface, .. } = ctx;
        let PunchState {
            peer_nat_type,
            is_local,
            signed_id_pk,
            relay_server,
            peer_addr,
            feedback,
            webrtc_sdp_answer,
            mut pending_webrtc_ice,
        } = state;
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
