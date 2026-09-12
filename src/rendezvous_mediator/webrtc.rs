use super::*;

impl RendezvousMediator {
    /// Build the WebRTC answerer for a punch-hole offer and return the SDP answer that rides in
    /// the punch reply (PunchHoleSent / RelayResponse).
    ///
    /// Awaited inline on the punch-reply path, which only holds because everything here is local
    /// (pc + keygen + SDP; trickle means the answer carries no candidates). Keep network I/O out
    /// — connection setup belongs in the detached task below.
    pub(super) async fn spawn_webrtc_answerer(
        &self,
        ph: &PunchHole,
        relay_only_ice: bool,
        server: ServerPtr,
        peer_addr: SocketAddr,
        meta: ConnectionMeta,
    ) -> ResultType<String> {
        let mut stream =
            WebRTCStream::new(&ph.webrtc_sdp_offer, relay_only_ice, CONNECT_TIMEOUT).await?;
        let answer = stream.local_endpoint().to_owned();
        let session_key = stream.session_key().to_owned();
        let return_route = ph.socket_addr.clone();

        // A duplicate PunchHole (the offerer re-sends the same request across punch attempts)
        // resolves to the SESSIONS-cached stream. `take_local_ice_rx` yields the receiver
        // exactly once per stream instance, so `None` here means an answerer was already
        // spawned for this offer: return the (identical) cached answer without spawning a
        // second connect task. Otherwise two `create_tcp_connection` tasks would detach and
        // read the same data channel, interleaving the handshake and corrupting the session.
        let Some(mut local_ice_rx) = stream.take_local_ice_rx() else {
            return Ok(answer);
        };

        // Bounded: how many candidates arrive is the sender's choice, while draining one costs a
        // JSON parse and the ICE agent's lock, so an unbounded queue lets whoever can reach this
        // session's route grow it without limit inside a long-lived service process. A full queue
        // drops the newest candidate, and the controller re-sends it once — the digests beside the
        // sender are what keep that re-send from spending a slot of its own.
        let (remote_ice_tx, mut remote_ice_rx) = mpsc::channel::<String>(MAX_PENDING_REMOTE_ICE);
        let own_ice_tx = remote_ice_tx.clone();
        WEBRTC_ICE_TXS
            .lock()
            .await
            .insert(session_key.clone(), IceRoute::new(remote_ice_tx));

        let stream_for_remote_ice = stream.clone();
        tokio::spawn(async move {
            while let Some(candidate) = remote_ice_rx.recv().await {
                if let Err(err) = stream_for_remote_ice.add_remote_ice_candidate(&candidate).await
                {
                    if let Some(n) = REJECTED_REMOTE_ICE_LOG.due() {
                        log::warn!(
                            "failed to add {} remote WebRTC ICE candidate(s), last: {}",
                            n,
                            err
                        );
                    }
                }
            }
        });

        {
            let host = self.host.clone();
            let socket_addr = return_route.clone();
            let session_key_for_ice = session_key.clone();
            tokio::spawn(async move {
                // Candidates ride a dedicated TCP connection to the rendezvous server, like
                // the answer, NOT the mediator channel: that channel is UDP in the default
                // setup, and target deployments front hbbs with websocket/TCP only, where
                // its UDP port is unreachable. The server keeps candidate-carrying TCP
                // connections open, so one lazily-opened connection serves the whole
                // trickle, and TCP reliability replaces the old 400ms duplicate re-send
                // (the controller keeps its own re-send for the server->peer UDP downlink).
                let mut conn = None;
                while let Some(candidate) = local_ice_rx.recv().await {
                    let mut msg = Message::new();
                    msg.set_ice_candidate(IceCandidate {
                        socket_addr: socket_addr.clone(),
                        session_key: session_key_for_ice.clone(),
                        candidate,
                        ..Default::default()
                    });
                    // One reconnect attempt per candidate: the first send after an hbbs
                    // restart or an idle-killed connection fails on the stale stream.
                    for _ in 0..2 {
                        if conn.is_none() {
                            match connect_tcp(&*host, CONNECT_TIMEOUT).await {
                                Ok(s) => conn = Some(s),
                                Err(err) => {
                                    log::warn!(
                                        "failed to connect for WebRTC ICE candidate: {}",
                                        err
                                    );
                                    break;
                                }
                            }
                        }
                        if let Some(s) = conn.as_mut() {
                            match s.send(&msg).await {
                                Ok(()) => break,
                                Err(err) => {
                                    log::debug!(
                                        "WebRTC ICE candidate send failed, reconnecting: {}",
                                        err
                                    );
                                    conn = None;
                                }
                            }
                        }
                    }
                }
            });
        }

        let session_key_for_cleanup = session_key.clone();
        tokio::spawn(async move {
            let result = stream.wait_connected(CONNECT_TIMEOUT).await;
            // Only evict our own route. The key is the offer's DTLS fingerprint, identical across
            // the controller's punch retries, so a retry that built a fresh answerer has already
            // replaced this entry — removing it blindly would delete the live session's sender and
            // leave it receiving no candidates at all.
            {
                let mut txs = WEBRTC_ICE_TXS.lock().await;
                if txs
                    .get(&session_key_for_cleanup)
                    .is_some_and(|route| route.is_same_channel(&own_ice_tx))
                {
                    txs.remove(&session_key_for_cleanup);
                }
            }
            if let Err(err) = result {
                log::warn!("webrtc wait_connected failed: {}", err);
                // Release the pc now rather than waiting for the ICE agent to time out into a
                // terminal state (~30s); this also drops the SESSIONS entry promptly.
                stream.close().await;
                return;
            }
            // create_tcp_connection takes ownership of the stream; keep a handle to close the pc
            // once the session returns. It runs the whole session and returns Ok on normal end,
            // Err on setup failure — either way the pc must be closed, else it lingers forever in
            // SESSIONS (its state handler only fires on a terminal ICE state, which a cleanly
            // closed session may never reach) leaking the pc, channels, and socket fds.
            let stream_for_cleanup = stream.clone();
            if let Err(err) = crate::server::create_tcp_connection(
                server,
                Stream::WebRTC(stream),
                peer_addr,
                true,
                meta,
            )
            .await
            {
                log::warn!("failed to create WebRTC server connection: {}", err);
            }
            stream_for_cleanup.close().await;
        });

        Ok(answer)
    }
}
