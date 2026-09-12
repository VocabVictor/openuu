use super::*;

impl Client {
    pub(super) const CLIENT_CLIPBOARD_NAME: &'static str = "client-clipboard";

    /// Start a new connection.
    pub async fn start(
        peer: &str,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
    ) -> ResultType<(
        (
            Stream,
            bool,
            Option<Vec<u8>>,
            Option<KcpStream>,
            &'static str,
        ),
        (i32, String),
    )> {
        crate::account::require_login().await?;
        let account_token = crate::account::session_token();
        let token = account_token.as_str();
        debug_assert!(peer == interface.get_id());
        interface.update_direct(None);
        interface.update_received(false);
        match Self::_start(peer, key, token, conn_type, interface.clone()).await {
            Err(err) => {
                let err_str = err.to_string();
                if err_str.starts_with("Failed") {
                    bail!(err_str + ": Please try later");
                } else {
                    return Err(err);
                }
            }
            Ok(x) => {
                // Set x.2 to true only in the connect() function to indicate that direct_failures needs to be updated; everywhere else it should be set to false.
                if x.2 {
                    let direct_failures = interface.get_lch().read().unwrap().direct_failures;
                    let direct = x.0 .1;
                    if !interface.is_force_relay() && (direct_failures == 0) != direct {
                        let n = if direct { 0 } else { 1 };
                        log::info!("direct_failures updated to {}", n);
                        interface.get_lch().write().unwrap().set_direct_failure(n);
                    }
                }
                Ok((x.0, x.1))
            }
        }
    }

    /// Start a new connection.
    pub(super) async fn _start(
        peer: &str,
        key: &str,
        token: &str,
        conn_type: ConnType,
        interface: impl Interface,
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
        if config::is_incoming_only() && !is_switch_sides_back(conn_type, &interface).await {
            bail!("Incoming only mode");
        }
        // to-do: remember the port for each peer, so that we can retry easier
        if hbb_common::is_ip_str(peer) {
            return Ok((
                (
                    connect_tcp_local(check_port(peer, RELAY_PORT + 1), None, CONNECT_TIMEOUT)
                        .await?,
                    true,
                    None,
                    None,
                    "TCP",
                ),
                (0, "".to_owned()),
                false,
            ));
        }
        // Allow connect to {domain}:{port}
        if hbb_common::is_domain_port_str(peer) {
            return Ok((
                (
                    connect_tcp_local(peer, None, CONNECT_TIMEOUT).await?,
                    true,
                    None,
                    None,
                    "TCP",
                ),
                (0, "".to_owned()),
                false,
            ));
        }

        let other_server = interface.get_lch().read().unwrap().other_server.clone();
        let (peer, other_server, key, token) = if let Some((a, b, c)) = other_server.as_ref() {
            (a.as_ref(), b.as_ref(), c.as_ref(), "")
        } else {
            (peer, "", key, token)
        };
        let (rendezvous_server, servers, contained) = if other_server.is_empty() {
            crate::get_rendezvous_server(1_000).await
        } else {
            if other_server == PUBLIC_SERVER {
                (
                    check_port(RENDEZVOUS_SERVERS[0], RENDEZVOUS_PORT),
                    RENDEZVOUS_SERVERS[1..]
                        .iter()
                        .map(|x| x.to_string())
                        .collect(),
                    true,
                )
            } else {
                (check_port(other_server, RENDEZVOUS_PORT), Vec::new(), true)
            }
        };

        // Same relay gate as the v6 socket below: under any forced relay the v6 punch cannot
        // be used, so probing v6 reachability is wasted work on every such connection.
        if crate::get_ipv6_punch_enabled() && !interface.is_force_relay() {
            crate::test_ipv6().await;
        }

        let (stop_udp_tx, stop_udp_rx) = oneshot::channel::<()>();
        let udp =
        // no need to care about multiple rendezvous servers case, since it is acutally not used any more.
        // Shared state for UDP NAT test result
        if crate::get_udp_punch_enabled() && !interface.is_force_relay() {
            if let Ok((socket, addr)) = new_direct_udp_for(&rendezvous_server).await {
                let udp_port = Arc::new(Mutex::new(0));
                let up_cloned = udp_port.clone();
                let socket_cloned = socket.clone();
                let func = async move {
                    allow_err!(test_udp_uat(socket_cloned, addr, up_cloned, stop_udp_rx).await);
                };
                tokio::spawn(func);
                (Some(socket), Some(udp_port))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        // Under force-relay a direct IPv6 path is not allowed, so don't bind the v6 socket;
        // the controlled side likewise skips v6 when relaying.
        let ipv6 = if crate::get_ipv6_punch_enabled() && !interface.is_force_relay() {
            crate::get_ipv6_socket().await
        } else {
            None
        };
        // WebRTC uses its own ICE sockets and does not depend on the legacy UDP punch socket.
        // When this request carries an offer, `_start_inner` keeps its rendezvous socket solely
        // for trickle signaling; a separate offer-less request owns any TCP punch attempt.
        let webrtc_offerer = if Self::should_create_webrtc_offerer(&interface) {
            // ICE policy follows relay-by-POLICY, not force_relay: under WebSocket the
            // latter is set for the classic paths, but ICE opens its own sockets and may
            // still go direct - that is the only P2P path ws deployments have.
            match WebRTCStream::new("", interface.is_policy_relay(), CONNECT_TIMEOUT).await {
                Ok(stream) => Some(stream),
                Err(err) => {
                    log::warn!("webrtc offerer setup failed: {}", err);
                    None
                }
            }
        } else {
            None
        };
        let has_webrtc_offerer = webrtc_offerer.is_some();
        let fut = Self::_start_inner(
            peer.to_owned(),
            key.to_owned(),
            token.to_owned(),
            conn_type,
            interface.clone(),
            udp.clone(),
            Some(stop_udp_tx),
            ipv6,
            webrtc_offerer,
            rendezvous_server.clone(),
            servers.clone(),
            contained,
        );
        // The fallback request exists only to carry a TCP punch, so it is pointless once TCP
        // punch is off — it would reach `connect()` with nothing to try and just open a second
        // relay.
        if interface.is_force_relay()
            || (udp.0.is_none() && !has_webrtc_offerer)
            || !tcp_punch_allowed()
        {
            return fut.await;
        }
        let preferred_fut = fut.boxed();
        // This is deliberately a pure TCP punch request: its WebRTC argument must stay `None`.
        // TCP punching closes the rendezvous socket before binding a new connection to the same
        // local address; a WebRTC ICE bridge would retain that socket and break the port reuse.
        // The preferred request retains its own socket for WebRTC signaling.
        let fallback_fut = Self::_start_inner(
            peer.to_owned(),
            key.to_owned(),
            token.to_owned(),
            conn_type,
            interface,
            (None, None),
            None,
            None,
            None,
            rendezvous_server,
            servers,
            contained,
        )
        .boxed();
        if has_webrtc_offerer {
            return race_transports_prefer_webrtc(
                preferred_fut,
                vec![fallback_fut],
                Self::WEBRTC_PREFER_WINDOW_MS,
                |result| result.0 .1,
            )
            .await;
        }
        let connect_futures = vec![preferred_fut, fallback_fut];
        match select_ok(connect_futures).await {
            Ok(conn) => Ok((conn.0 .0, conn.0 .1, conn.0 .2)),
            Err(e) => Err(e),
        }
    }
}
