use super::*;

/// What `_start_inner` hands back to the connect race.
pub(super) type StartResult = (
    (Stream, bool, Option<Vec<u8>>, Option<KcpStream>, &'static str),
    (i32, String),
    bool,
);

/// The read-only inputs of one `_start_inner` run, shared by its stages.
pub(super) struct StartCtx<'a, I: Interface> {
    pub(super) peer: &'a str,
    pub(super) key: &'a str,
    pub(super) token: &'a str,
    pub(super) conn_type: ConnType,
    pub(super) interface: I,
    pub(super) rendezvous_server: &'a str,
    pub(super) my_addr: SocketAddr,
    /// When the punch round began; relay timing is measured from it.
    pub(super) start: Instant,
}

impl Client {
    /// The rendezvous server answered with a relay: race the relay against any IPv6 or WebRTC
    /// path still open, secure the winner and return it as the connection.
    pub(super) async fn connect_on_relay_response<I: Interface>(
        rr: RelayResponse,
        socket: Stream,
        ipv6_socket: Option<Arc<UdpSocket>>,
        mut webrtc_offerer: Option<OffererGuard>,
        mut pending_webrtc_ice: Vec<String>,
        ctx: StartCtx<'_, I>,
    ) -> ResultType<StartResult> {
        let mut start = ctx.start;
        log::info!(
            "relay requested from peer, time used: {:?}, relay_server: {}",
            start.elapsed(),
            rr.relay_server
        );
        start = Instant::now();
        let mut connect_futures = Vec::new();
        if let Some(s) = ipv6_socket {
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
        let signed_id_pk: Vec<u8> = rr.pk().into();
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
                        ctx.peer.to_owned(),
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
            ctx.peer,
            rr.uuid,
            rr.relay_server,
            ctx.key,
            ctx.conn_type,
            ctx.my_addr.is_ipv4(),
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
            if ctx.interface.is_policy_relay() {
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
        let feedback = rr.feedback;
        log::info!("{:?} used to establish {typ} connection", start.elapsed());
        let pk = match Self::secure_connection(
            ctx.peer,
            signed_id_pk.clone(),
            ctx.key,
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
                    ctx.peer,
                    relay_server_rr,
                    ctx.rendezvous_server,
                    !signed_id_pk.is_empty(),
                    ctx.key,
                    ctx.token,
                    ctx.conn_type,
                    &ctx.interface.get_switch_code(),
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
                    ctx.peer,
                    signed_id_pk,
                    ctx.key,
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
            (feedback, ctx.rendezvous_server.to_owned()),
            false,
        ));
    }
}
