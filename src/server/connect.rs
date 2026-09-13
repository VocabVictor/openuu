use super::*;

async fn accept_connection_(
    server: ServerPtr,
    socket: Stream,
    secure: bool,
    meta: ConnectionMeta,
) -> ResultType<()> {
    let local_addr = socket.local_addr();
    drop(socket);
    // even we drop socket, below still may fail if not use reuse_addr,
    // there is TIME_WAIT before socket really released, so sometimes we
    // see "Only one usage of each socket address is normally permitted" on windows sometimes,
    let listener = new_listener(local_addr, true).await?;
    log::info!("Server listening on: {}", &listener.local_addr()?);
    if let Ok((stream, addr)) = timeout(CONNECT_TIMEOUT, listener.accept()).await? {
        stream.set_nodelay(true).ok();
        let stream_addr = stream.local_addr()?;
        create_tcp_connection(
            server,
            Stream::from(stream, stream_addr),
            addr,
            secure,
            meta,
        )
        .await?;
    }
    Ok(())
}

pub async fn create_tcp_connection(
    server: ServerPtr,
    stream: Stream,
    addr: SocketAddr,
    secure: bool,
    meta: ConnectionMeta,
) -> ResultType<()> {
    let mut stream = stream;
    let id = server.write().unwrap().get_new_id();
    let (sk, pk) = Config::get_key_pair();
    if secure && pk.len() == sign::PUBLICKEYBYTES && sk.len() == sign::SECRETKEYBYTES {
        let mut sk_ = [0u8; sign::SECRETKEYBYTES];
        sk_[..].copy_from_slice(&sk);
        let sk = sign::SecretKey(sk_);
        let mut msg_out = Message::new();
        let (our_pk_b, our_sk_b) = box_::gen_keypair();
        // On a WebRTC transport, bind our DTLS certificate fingerprint to our signed identity so
        // the controller can verify the DTLS channel it negotiated actually terminates at us
        // (not a rendezvous/relay that swapped the SDP fingerprint). Empty on other transports.
        // Fail immediately on WebRTC if the local fingerprint is unavailable: signing "" would
        // only make the client fail-closed after a wasted round-trip.
        let dtls_fingerprint = stream.dtls_fingerprint(true).await.unwrap_or_default();
        if stream.is_webrtc() && dtls_fingerprint.is_empty() {
            bail!("WebRTC local DTLS fingerprint unavailable");
        }
        msg_out.set_signed_id(SignedId {
            id: sign::sign(
                &IdPk {
                    id: Config::get_id(),
                    pk: Bytes::from(our_pk_b.0.to_vec()),
                    dtls_fingerprint,
                    ..Default::default()
                }
                .write_to_bytes()
                .unwrap_or_default(),
                &sk,
            )
            .into(),
            ..Default::default()
        });
        timeout(CONNECT_TIMEOUT, stream.send(&msg_out)).await??;
        match timeout(CONNECT_TIMEOUT, stream.next()).await? {
            Some(res) => {
                let bytes = res?;
                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                    if let Some(message::Union::PublicKey(pk)) = msg_in.union {
                        if pk.asymmetric_value.len() == box_::PUBLICKEYBYTES {
                            stream.set_key(tcp::Encrypt::decode(
                                &pk.symmetric_value,
                                &pk.asymmetric_value,
                                &our_sk_b,
                            )?);
                        } else if pk.asymmetric_value.is_empty() {
                            Config::set_key_confirmed(false);
                            log::info!("Force to update pk");
                        } else {
                            bail!("Handshake failed: invalid public sign key length from peer");
                        }
                    } else {
                        log::error!("Handshake failed: invalid message type");
                    }
                } else {
                    bail!("Handshake failed: invalid message format");
                }
            }
            None => {
                bail!("Failed to receive public key");
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(task) = Command::new("/usr/bin/caffeinate")
            .arg("-u")
            .arg("-t 5")
            .spawn()
        {
            super::add_child(task);
        }
        log::info!("wake up macos");
    }
    Connection::start(addr, stream, id, Arc::downgrade(&server), meta).await;
    Ok(())
}

pub async fn accept_connection(
    server: ServerPtr,
    socket: Stream,
    peer_addr: SocketAddr,
    secure: bool,
    meta: ConnectionMeta,
) {
    if let Err(err) = accept_connection_(server, socket, secure, meta).await {
        log::warn!("Failed to accept connection from {}: {}", peer_addr, err);
    }
}

pub async fn create_relay_connection(
    server: ServerPtr,
    relay_server: String,
    uuid: String,
    peer_addr: SocketAddr,
    secure: bool,
    ipv4: bool,
    meta: ConnectionMeta,
    peer_ticket: String,
) {
    if let Err(err) = create_relay_connection_(
        server,
        relay_server,
        uuid.clone(),
        peer_addr,
        secure,
        ipv4,
        meta,
        peer_ticket,
    )
    .await
    {
        log::error!(
            "Failed to create relay connection for {} with uuid {}: {}",
            peer_addr,
            uuid,
            err
        );
    }
}

async fn create_relay_connection_(
    server: ServerPtr,
    relay_server: String,
    uuid: String,
    peer_addr: SocketAddr,
    secure: bool,
    ipv4: bool,
    meta: ConnectionMeta,
    peer_ticket: String,
) -> ResultType<()> {
    let mut stream = socket_client::connect_tcp(
        socket_client::ipv4_to_ipv6(crate::check_port(relay_server, RELAY_PORT), ipv4),
        CONNECT_TIMEOUT,
    )
    .await?;
    let mut msg_out = RendezvousMessage::new();
    let licence_key = crate::get_key(true).await;
    msg_out.set_request_relay(RequestRelay {
        token: relay_token(peer_ticket, &uuid).await?,
        licence_key,
        uuid,
        ..Default::default()
    });
    stream.send(&msg_out).await?;
    create_tcp_connection(server, stream, peer_addr, secure, meta).await?;
    Ok(())
}

/// The ticket this side presents to hbbr. A new hbbs forwards one inside RequestRelay so an
/// unattended peer needs no account (docs/relay-ticket-peer.md in openuu-server); an old hbbs
/// leaves it empty and the peer fetches its own ticket with its login, as before.
async fn relay_token(peer_ticket: String, uuid: &str) -> ResultType<String> {
    if !peer_ticket.is_empty() {
        return Ok(peer_ticket);
    }
    crate::account::require_login().await?;
    crate::account::relay_ticket(uuid).await
}

#[cfg(test)]
mod tests {
    use super::relay_token;

    #[hbb_common::tokio::test]
    async fn forwarded_ticket_is_used_as_is() {
        let ticket = "f".repeat(64);
        assert_eq!(relay_token(ticket.clone(), "uuid-1").await.unwrap(), ticket);
    }

    #[hbb_common::tokio::test]
    async fn empty_ticket_falls_back_to_this_peer_login() {
        // No account is configured in the test environment, so the fallback is the login check.
        let err = relay_token(String::new(), "uuid-1").await.unwrap_err();
        assert!(err.to_string().contains("Sign in"), "{err}");
    }
}
