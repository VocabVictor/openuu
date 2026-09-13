use super::*;

impl RendezvousMediator {
    pub(super) async fn handle_punch_hole(&self, ph: PunchHole, server: ServerPtr) -> ResultType<()> {
        let mut peer_addr = AddrMangle::decode(&ph.socket_addr);
        let last = *LAST_MSG.lock().await;
        *LAST_MSG.lock().await = (peer_addr, Instant::now());
        // skip duplicate punch hole messages
        if last.0 == peer_addr && last.1.elapsed().as_millis() < 100 {
            return Ok(());
        }
        let peer_addr_v6 = hbb_common::AddrMangle::decode(&ph.socket_addr_v6);
        let local_proxy = use_ws() || Config::is_proxy();
        let relay = local_proxy || ph.force_relay;
        let mut socket_addr_v6 = Default::default();
        let meta = connection_meta(
            ph.control_permissions.clone().into_option(),
            ph.controlled_context.clone().into_option(),
        );
        // The controller's force_relay alone does not say whether ICE must be Relay-only; its
        // offer envelope does. `ice_policy: "all"` means the relay was forced by the transport
        // (ws), so answer with full ICE and let a direct pair form.
        let webrtc_relay_only =
            ph.force_relay && !WebRTCStream::endpoint_declares_all_ice(&ph.webrtc_sdp_offer);
        // No enable-webrtc check here: it is LocalConfig, which the UI process writes and never
        // syncs over IPC, so this (server) process would read the private-server default of "N"
        // and refuse to answer in exactly the self-hosted deployments the transport is for.
        // A proxy still rules it out — ICE would bypass it and leak the real IP.
        let webrtc_viable = !ph.webrtc_sdp_offer.is_empty()
            && !Config::is_proxy()
            && (!webrtc_relay_only || WebRTCStream::has_turn_server());
        let webrtc_sdp_answer = if webrtc_viable {
            self.spawn_webrtc_answerer(
                &ph,
                webrtc_relay_only,
                server.clone(),
                peer_addr,
                meta.clone(),
            )
            .await
            .unwrap_or_else(|err| {
                log::warn!("failed to create WebRTC answer: {}", err);
                String::new()
            })
        } else {
            String::new()
        };
        if peer_addr_v6.port() > 0 && !relay {
            socket_addr_v6 =
                start_ipv6(peer_addr_v6, peer_addr, server.clone(), meta.clone()).await;
        }
        let relay_server = self.get_relay_server(ph.relay_server);
        // for ensure, websocket go relay directly
        // A symmetric NAT relays the legacy transports but deliberately not WebRTC: the answer
        // built above rides along on the relay request, and ICE probes the candidate pairs rather
        // than trusting this classification, so a direct WebRTC pair can still form on a
        // connection this branch has already called relay-only. Do not gate the answerer on
        // nat_type to make the two agree.
        if ph.nat_type.enum_value() == Ok(NatType::SYMMETRIC)
            || Config::get_nat_type() == NatType::SYMMETRIC as i32
            || relay
            || (config::is_disable_tcp_listen() && ph.udp_port <= 0)
        {
            let uuid = Uuid::new_v4().to_string();
            return self
                .create_relay(
                    ph.socket_addr.into(),
                    relay_server,
                    uuid,
                    server,
                    true,
                    true,
                    socket_addr_v6.clone(),
                    webrtc_sdp_answer.clone(),
                    String::new(),
                    meta,
                )
                .await;
        }
        use hbb_common::protobuf::Enum;
        let nat_type = NatType::from_i32(Config::get_nat_type()).unwrap_or(NatType::UNKNOWN_NAT);
        let msg_punch = PunchHoleSent {
            socket_addr: ph.socket_addr,
            id: Config::get_id(),
            relay_server,
            nat_type: nat_type.into(),
            version: crate::VERSION.to_owned(),
            socket_addr_v6,
            webrtc_sdp_answer,
            ..Default::default()
        };
        if ph.udp_port > 0 {
            peer_addr.set_port(ph.udp_port as u16);
            self.punch_udp_hole(peer_addr, server, msg_punch, meta)
                .await?;
            return Ok(());
        }
        if answers_webrtc_only(&msg_punch.webrtc_sdp_answer) {
            // Return the answer over its own short-lived TCP connection rather than the mediator
            // channel: that channel is UDP by default, and hbbs applies UDP-punch semantics
            // (source-address observation) to a PunchHoleSent that arrives on it. No TCP punch
            // is made — the controller keeps its request socket for trickled ICE.
            //
            // Only with an answer. An offer this side could not answer (proxy, relay-only
            // without TURN, answerer failure) still gets the TCP punch below: hbbs hands the
            // controller the address of whatever socket carried PunchHoleSent, and the
            // controller dials it, so that socket has to be the one a listener takes over.
            let mut msg_out = Message::new();
            msg_out.set_punch_hole_sent(msg_punch);
            let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
            socket.send(&msg_out).await?;
            return Ok(());
        }
        log::debug!("Punch tcp hole to {:?}", peer_addr);
        let mut socket = {
            let socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
            let local_addr = socket.local_addr();
            // key important here for punch hole to tell my gateway incoming peer is safe.
            // Awaited rather than spawned so the mapping exists before `PunchHoleSent` goes out;
            // `local_addr` itself is shared, not exclusive - every socket here binds it with the
            // reuse flags `new_socket` sets.
            allow_err!(socket_client::connect_tcp_local(peer_addr, Some(local_addr), 30).await);
            socket
        };
        let mut msg_out = Message::new();
        msg_out.set_punch_hole_sent(msg_punch);
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        let local_addr = socket.local_addr();
        // The listener inside takes this address over, so the mediator's socket goes first.
        drop(socket);
        punch_tcp_until_connected(server, peer_addr, local_addr, meta).await;
        Ok(())
    }

    pub(super) async fn punch_udp_hole(
        &self,
        peer_addr: SocketAddr,
        server: ServerPtr,
        msg_punch: PunchHoleSent,
        meta: ConnectionMeta,
    ) -> ResultType<()> {
        let mut msg_out = Message::new();
        msg_out.set_punch_hole_sent(msg_punch);
        let (socket, addr) = new_direct_udp_for(&self.host).await?;
        let data = msg_out.write_to_bytes()?;
        socket.send_to(&data, addr).await?;
        let socket_cloned = socket.clone();
        tokio::spawn(async move {
            for _ in 0..2 {
                let tm = (hbb_common::time_based_rand() % 20 + 10) as f32 / 1000.;
                hbb_common::sleep(tm).await;
                socket.send_to(&data, addr).await.ok();
            }
        });
        udp_nat_listen(socket_cloned.clone(), peer_addr, peer_addr, server, meta).await?;
        Ok(())
    }
}

/// Whether the punch ends with the WebRTC answer alone, leaving no TCP listener behind.
pub(super) fn answers_webrtc_only(webrtc_sdp_answer: &str) -> bool {
    !webrtc_sdp_answer.is_empty()
}
