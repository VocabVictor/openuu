use super::*;

impl Client {
    /// Connect to the peer.
    pub(super) async fn connect(
        local_addr: SocketAddr,
        peer: SocketAddr,
        peer_id: &str,
        signed_id_pk: Vec<u8>,
        relay_server: &str,
        rendezvous_server: &str,
        punch_time_used: u64,
        peer_nat_type: NatType,
        my_nat_type: i32,
        is_local: bool,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
        udp_socket_nat: Option<Arc<UdpSocket>>,
        udp_socket_v6: Option<Arc<UdpSocket>>,
        webrtc_offerer: Option<WebRTCStream>,
        webrtc_bridge_stop: Option<oneshot::Sender<()>>,
        allow_tcp_punch: bool,
        punch_type: &str,
    ) -> ResultType<(
        Stream,
        bool,
        Option<Vec<u8>>,
        Option<KcpStream>,
        &'static str,
    )> {
        // Guard the offerer for the whole of connect(): any early return — cancellation during the
        // awaits below, the relay override, or a secure_connection failure — closes its pc via the
        // guard's drop. Disarmed only once WebRTC is the confirmed winning, secured transport.
        let mut webrtc_guard = webrtc_offerer.map(OffererGuard::new);
        let direct_failures = interface.get_lch().read().unwrap().direct_failures;
        let mut connect_timeout = 0;
        const MIN: u64 = 1000;
        if is_local || peer_nat_type == NatType::SYMMETRIC {
            connect_timeout = MIN;
        } else {
            if relay_server.is_empty() {
                connect_timeout = CONNECT_TIMEOUT;
            } else {
                if peer_nat_type == NatType::ASYMMETRIC {
                    let mut my_nat_type = my_nat_type;
                    if my_nat_type == NatType::UNKNOWN_NAT as i32 {
                        my_nat_type = crate::get_nat_type(100).await;
                    }
                    if my_nat_type == NatType::ASYMMETRIC as i32 {
                        connect_timeout = CONNECT_TIMEOUT;
                        if direct_failures > 0 {
                            connect_timeout = punch_time_used * 6;
                        }
                    } else if my_nat_type == NatType::SYMMETRIC as i32 {
                        connect_timeout = MIN;
                    }
                }
                if connect_timeout == 0 {
                    let n = if direct_failures > 0 { 3 } else { 6 };
                    connect_timeout = punch_time_used * (n as u64);
                }
            }
            if connect_timeout < MIN {
                connect_timeout = MIN;
            }
        }
        log::info!("peer address: {}, timeout: {}", peer, connect_timeout);
        let start = std::time::Instant::now();

        // Each attempt carries whether its path is direct (4th field). TCP/UDP/IPv6 punch are
        // always direct; WebRTC is direct only when ICE nominated a non-TURN pair.
        let mut direct_futures = Vec::new();
        if allow_tcp_punch {
            let fut = connect_tcp_local(peer, Some(local_addr), connect_timeout);
            direct_futures.push(
                async move {
                    let conn = fut.await?;
                    Ok((conn, None, "TCP", true))
                }
                .boxed(),
            );
        }
        if let Some(udp_socket_nat) = udp_socket_nat {
            direct_futures.push(
                async move {
                    let (conn, kcp, typ) =
                        udp_nat_connect(udp_socket_nat, "UDP", connect_timeout).await?;
                    Ok((conn, kcp, typ, true))
                }
                .boxed(),
            );
        }
        if let Some(udp_socket_v6) = udp_socket_v6 {
            direct_futures.push(
                async move {
                    let (conn, kcp, typ) =
                        udp_nat_connect(udp_socket_v6, "IPv6", connect_timeout).await?;
                    Ok((conn, kcp, typ, true))
                }
                .boxed(),
            );
        }
        // Race a clone of the offerer; the guard retains its own clone so a losing/cancelled race
        // still closes the pc (select_ok drops the future's clone without closing).
        let webrtc_fut = webrtc_guard
            .as_ref()
            .and_then(|g| g.stream())
            .map(|stream| {
                let mut raced = stream.clone();
                // The punch-tuned timeout can be as low as 1s — enough for a raw TCP SYN but not
                // for candidate trickle + ICE checks + DTLS. Give WebRTC its own floor (prefer-P2P)
                // so a viable P2P path is not abandoned before it can complete; TCP/UDP keep the
                // tighter timeout, so a working direct connection still wins immediately, and the
                // relay fallback only waits the extra time when direct attempts all failed.
                let webrtc_timeout = connect_timeout.max(Self::WEBRTC_PREFER_WINDOW_MS);
                async move {
                    raced.wait_connected(webrtc_timeout).await?;
                    // Resolve the pair here: a TURN win is relayed, not direct, and must be held
                    // behind still-racing direct attempts rather than committed as P2P.
                    let relayed = raced.is_relayed().await.unwrap_or(true);
                    Ok((Stream::WebRTC(raced), None, "WebRTC", !relayed))
                }
                .boxed()
            });
        // Prefer P2P: a direct result wins outright, a relayed WebRTC (TURN) is held for the
        // window so a direct punch can still land. Falls back to plain select_ok when only one
        // kind is present.
        let direct_result = match (webrtc_fut, direct_futures.is_empty()) {
            (Some(webrtc_fut), false) => {
                race_transports_prefer_webrtc(
                    webrtc_fut,
                    direct_futures,
                    Self::WEBRTC_PREFER_WINDOW_MS,
                    |r| r.3,
                )
                .await
            }
            (Some(webrtc_fut), true) => webrtc_fut.await,
            (None, false) => select_ok(direct_futures).await.map(|c| c.0),
            (None, true) => Err(anyhow!("No direct transport available")),
        };
        let (mut conn, kcp, mut typ, mut direct) = match direct_result {
            Ok((conn, kcp, typ, direct)) => (Ok(conn), kcp, typ, direct),
            Err(e) => (Err(e), None, "", false),
        };
        if let Some(stop) = webrtc_bridge_stop {
            let _ = stop.send(());
        }
        // webrtc_guard stays armed across the relay override and secure_connection below; it is
        // disarmed only at the successful return when WebRTC is the kept transport.

        // Keep a WebRTC win instead of replacing it with the RustDesk relay: under relay-by-
        // policy the pc was built with Relay-only ICE (TURN configured), which already honors
        // the relay requirement, and under ws-forced relay a direct full-ICE connection is the
        // preferred outcome, not a violation.
        if (interface.is_force_relay() && typ != "WebRTC") || conn.is_err() {
            if !relay_server.is_empty() {
                let switch_code = interface.get_switch_code();
                conn = Self::request_relay(
                    peer_id,
                    relay_server.to_owned(),
                    rendezvous_server,
                    !signed_id_pk.is_empty(),
                    key,
                    token,
                    conn_type,
                    &switch_code,
                )
                .await;
                if let Err(e) = conn {
                    // this direct is mainly used by on_establish_connection_error, so we update it here before bail
                    interface.update_direct(Some(false));
                    bail!("Failed to connect via relay server: {}", e);
                }
                typ = "Relay";
                direct = false;
            } else {
                bail!("Failed to make direct connection to remote desktop");
            }
        }
        let mut conn = conn?;
        log::info!(
            "{:?} used to establish {typ} connection with {} punch",
            start.elapsed(),
            punch_type
        );
        let res = Self::secure_connection(peer_id, signed_id_pk.clone(), key, &mut conn).await;
        let pk: Option<Vec<u8>> = match res {
            Ok(pk) => pk,
            Err(e) if typ == "WebRTC" && !relay_server.is_empty() => {
                // WebRTC won the race but identity/DTLS binding failed; fall back to a freshly
                // coordinated relay instead of failing the whole attempt. The guard is dropped
                // first so the bad pc is closed promptly.
                log::warn!("WebRTC secure handshake failed ({}), falling back to relay", e);
                drop(webrtc_guard.take());
                match Self::request_relay(
                    peer_id,
                    relay_server.to_owned(),
                    rendezvous_server,
                    !signed_id_pk.is_empty(),
                    key,
                    token,
                    conn_type,
                    &interface.get_switch_code(),
                )
                .await
                {
                    Ok(mut relay_conn) => {
                        match Self::secure_connection(peer_id, signed_id_pk, key, &mut relay_conn)
                            .await
                        {
                            Ok(pk) => {
                                conn = relay_conn;
                                typ = "Relay";
                                direct = false;
                                pk
                            }
                            Err(e) => {
                                interface.update_direct(Some(false));
                                bail!(e);
                            }
                        }
                    }
                    Err(relay_e) => {
                        interface.update_direct(Some(direct));
                        bail!(
                            "WebRTC secure failed ({}); relay fallback also failed: {}",
                            e,
                            relay_e
                        );
                    }
                }
            }
            Err(e) => {
                // this direct is mainly used by on_establish_connection_error, so we update it here before bail
                interface.update_direct(Some(direct));
                // webrtc_guard is still armed here, so a WebRTC winner whose secure handshake
                // failed is closed by the guard's drop as we bail (no explicit close needed).
                bail!(e);
            }
        };
        if typ == "WebRTC" {
            // WebRTC through a TURN server (force_relay, or a TURN pair winning under All
            // policy) is relayed traffic; report the direct flag accordingly. An unknown answer
            // counts as relayed — claiming a P2P path needs evidence of one.
            if conn.webrtc_relayed().await.unwrap_or(true) {
                direct = false;
            }
            // Secured: disarm so the returned conn keeps the pc alive.
            if let Some(guard) = webrtc_guard.take() {
                let _ = guard.into_inner();
            }
        }
        log::debug!("{} punch secure_connection ok", punch_type);
        Ok((conn, direct, pk, kcp, typ))
    }
}
