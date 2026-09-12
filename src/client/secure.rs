use super::*;

impl Client {
    /// Establish secure connection with the server.
    pub(super) async fn secure_connection(
        peer_id: &str,
        signed_id_pk: Vec<u8>,
        key: &str,
        conn: &mut Stream,
    ) -> ResultType<Option<Vec<u8>>> {
        let rs_pk = get_rs_pk(if key.is_empty() {
            config::RS_PUB_KEY
        } else {
            key
        });
        // A WebRTC channel is peer-authenticated only once its DTLS fingerprint is bound to the
        // verified peer identity below. Once a trusted identity IS established, every binding
        // failure fails closed: any peer able to answer WebRTC also signs its fingerprint, so a
        // mismatch is concrete evidence of a rendezvous/relay MITM (not a legacy peer), and
        // callers fall back to a fresh relay connection rather than proceeding on that channel.
        // Without a trusted identity (absent, or unverifiable under our configured root) no
        // binding is possible at all; WebRTC then proceeds like TCP's non-secure fallback —
        // DTLS-encrypted but reported unsecured. TCP/UDP/relay keep their existing behavior.
        let is_webrtc = conn.is_webrtc();
        let mut sign_pk = None;
        let mut option_pk = None;
        if !signed_id_pk.is_empty() {
            if let Some(rs_pk) = rs_pk {
                if let Ok((id, pk)) = decode_id_pk(&signed_id_pk, &rs_pk) {
                    if id == peer_id {
                        sign_pk = Some(sign::PublicKey(pk));
                        option_pk = Some(pk.to_vec());
                    }
                }
            }
            if sign_pk.is_none() {
                log::error!("Handshake failed: invalid public key from rendezvous server");
            }
        }
        let sign_pk = match sign_pk {
            Some(v) => v,
            None => {
                // No trusted peer identity (key-less deployment, or a blob that does not verify
                // under our root), so no fingerprint binding is possible. Fall through like TCP's
                // non-secure path with is_secured() false: bailing would only move the session to
                // a relay that fails the same check and then runs in plaintext.
                // send an empty message out in case server is setting up secure and waiting for first message
                conn.send(&Message::new()).await?;
                return Ok(option_pk);
            }
        };
        match timeout(READ_TIMEOUT, conn.next()).await? {
            Some(res) => {
                let bytes = res?;
                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                    if let Some(message::Union::SignedId(si)) = msg_in.union {
                        if let Ok((id, their_pk_b, signed_fp)) = decode_id_pk_dtls(&si.id, &sign_pk) {
                            if id == peer_id {
                                // WebRTC only: bind the DTLS channel to the verified peer identity.
                                // webrtc-rs already bound the certificate to the remote SDP, so
                                // requiring the peer to have SIGNED that fingerprint defeats a
                                // rendezvous/relay MITM that swaps SDPs. Fail closed.
                                if is_webrtc {
                                    let actual_fp = conn.dtls_fingerprint(false).await.ok_or_else(
                                        || anyhow!("WebRTC DTLS fingerprint unavailable"),
                                    )?;
                                    if signed_fp.is_empty() || signed_fp != actual_fp {
                                        bail!("WebRTC DTLS fingerprint not bound to peer identity (possible MITM)");
                                    }
                                }
                                let (asymmetric_value, symmetric_value, key) =
                                    create_symmetric_key_msg(their_pk_b);
                                let mut msg_out = Message::new();
                                msg_out.set_public_key(PublicKey {
                                    asymmetric_value,
                                    symmetric_value,
                                    ..Default::default()
                                });
                                timeout(CONNECT_TIMEOUT, conn.send(&msg_out)).await??;
                                conn.set_key(key);
                            } else {
                                if is_webrtc {
                                    bail!("WebRTC handshake id mismatch (possible MITM)");
                                }
                                log::error!("Handshake failed: sign failure");
                                conn.send(&Message::new()).await?;
                            }
                        } else {
                            if is_webrtc {
                                bail!("WebRTC peer identity could not be verified (refusing unbound channel)");
                            }
                            // fall back to non-secure connection in case pk mismatch
                            log::info!("pk mismatch, fall back to non-secure");
                            let mut msg_out = Message::new();
                            msg_out.set_public_key(PublicKey::new());
                            conn.send(&msg_out).await?;
                        }
                    } else {
                        if is_webrtc {
                            bail!("WebRTC handshake received an unexpected message type");
                        }
                        log::error!("Handshake failed: invalid message type");
                        conn.send(&Message::new()).await?;
                    }
                } else {
                    if is_webrtc {
                        bail!("WebRTC handshake received a malformed message");
                    }
                    log::error!("Handshake failed: invalid message format");
                    conn.send(&Message::new()).await?;
                }
            }
            None => {
                bail!("Reset by the peer");
            }
        }
        Ok(option_pk)
    }

    /// Request a relay connection to the server.
    pub(super) async fn request_relay(
        peer: &str,
        relay_server: String,
        rendezvous_server: &str,
        secure: bool,
        key: &str,
        token: &str,
        conn_type: ConnType,
        switch_code: &str,
    ) -> ResultType<Stream> {
        let mut succeed = false;
        let mut uuid = "".to_owned();
        let mut ipv4 = true;

        for i in 1..=3 {
            // use different socket due to current hbbs implementation requiring different nat address for each attempt
            let mut socket = connect_tcp(rendezvous_server, CONNECT_TIMEOUT)
                .await
                .with_context(|| "Failed to connect to rendezvous server")?;

            if !key.is_empty() && (!token.is_empty() || !switch_code.is_empty()) {
                secure_tcp(&mut socket, key).await?;
            }

            ipv4 = socket.local_addr().is_ipv4();
            let mut msg_out = RendezvousMessage::new();
            uuid = Uuid::new_v4().to_string();
            log::info!(
                "#{} request relay attempt, id: {}, uuid: {}, relay_server: {}, secure: {}",
                i,
                peer,
                uuid,
                relay_server,
                secure,
            );
            msg_out.set_request_relay(RequestRelay {
                id: peer.to_owned(),
                token: token.to_owned(),
                uuid: uuid.clone(),
                relay_server: relay_server.clone(),
                secure,
                switch_code: switch_code.to_owned(),
                ..Default::default()
            });
            socket.send(&msg_out).await?;

            if let Some(msg_in) =
                crate::get_next_nonkeyexchange_msg(&mut socket, Some(CONNECT_TIMEOUT)).await
            {
                if let Some(rendezvous_message::Union::RelayResponse(rs)) = msg_in.union {
                    if !rs.refuse_reason.is_empty() {
                        bail!(rs.refuse_reason);
                    }
                    succeed = true;
                    break;
                }
            }
        }
        if !succeed {
            bail!("Timeout");
        }
        Self::create_relay(peer, uuid, relay_server, key, conn_type, ipv4).await
    }

    /// Create a relay connection to the server.
    pub(super) async fn create_relay(
        peer: &str,
        uuid: String,
        relay_server: String,
        key: &str,
        conn_type: ConnType,
        ipv4: bool,
    ) -> ResultType<Stream> {
        let mut conn = connect_tcp(
            ipv4_to_ipv6(check_port(relay_server, RELAY_PORT), ipv4),
            CONNECT_TIMEOUT,
        )
        .await
        .with_context(|| "Failed to connect to relay server")?;
        let mut msg_out = RendezvousMessage::new();
        msg_out.set_request_relay(RequestRelay {
            token: crate::account::relay_ticket(&uuid).await?,
            licence_key: key.to_owned(),
            id: peer.to_owned(),
            uuid,
            conn_type: conn_type.into(),
            ..Default::default()
        });
        conn.send(&msg_out).await?;
        Ok(conn)
    }
}
