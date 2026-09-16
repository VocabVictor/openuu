use super::*;

impl RendezvousMediator {
    pub(super) async fn handle_request_relay(&self, rr: RequestRelay, server: ServerPtr) -> ResultType<()> {
        let addr = AddrMangle::decode(&rr.socket_addr);
        let last = *LAST_RELAY_MSG.lock().await;
        *LAST_RELAY_MSG.lock().await = (addr, Instant::now());
        // skip duplicate relay request messages
        if last.0 == addr && last.1.elapsed().as_millis() < 100 {
            return Ok(());
        }
        let meta = connection_meta(
            rr.control_permissions.into_option(),
            rr.controlled_context.into_option(),
        );

        self.create_relay(
            rr.socket_addr.into(),
            rr.relay_server,
            rr.uuid,
            server,
            rr.secure,
            false,
            Default::default(),
            String::new(),
            rr.token,
            meta,
        )
        .await
    }

    pub(super) async fn create_relay(
        &self,
        socket_addr: Vec<u8>,
        relay_server: String,
        uuid: String,
        server: ServerPtr,
        secure: bool,
        initiate: bool,
        socket_addr_v6: bytes::Bytes,
        webrtc_sdp_answer: String,
        peer_ticket: String,
        meta: ConnectionMeta,
    ) -> ResultType<()> {
        let peer_addr = AddrMangle::decode(&socket_addr);
        log::info!(
            "create_relay requested from {:?}, relay_server: {}, uuid: {}, secure: {}",
            peer_addr,
            relay_server,
            uuid,
            secure,
        );

        let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;

        let mut msg_out = Message::new();
        let mut rr = RelayResponse {
            socket_addr: socket_addr.into(),
            version: crate::VERSION.to_owned(),
            socket_addr_v6,
            webrtc_sdp_answer,
            ..Default::default()
        };
        if initiate {
            rr.uuid = uuid.clone();
            rr.relay_server = relay_server.clone();
            rr.set_id(Config::get_id());
        }
        msg_out.set_relay_response(rr);
        socket.send(&msg_out).await?;
        let peer_ticket = if waits_for_peer_ticket(initiate, &peer_ticket) {
            peer_ticket_from_hbbs(&mut socket, &uuid).await
        } else {
            peer_ticket
        };
        crate::create_relay_connection(
            server,
            relay_server,
            uuid,
            peer_addr,
            secure,
            is_ipv4(&self.addr),
            meta,
            peer_ticket,
        )
        .await;
        Ok(())
    }

    pub(super) async fn handle_intranet(&self, fla: FetchLocalAddr, server: ServerPtr) -> ResultType<()> {
        let addr = AddrMangle::decode(&fla.socket_addr);
        let last = *LAST_MSG.lock().await;
        *LAST_MSG.lock().await = (addr, Instant::now());
        // skip duplicate punch hole messages
        if last.0 == addr && last.1.elapsed().as_millis() < 100 {
            return Ok(());
        }
        let peer_addr_v6 = hbb_common::AddrMangle::decode(&fla.socket_addr_v6);
        let relay_server = self.get_relay_server(fla.relay_server.clone());
        let relay = use_ws() || Config::is_proxy();
        let mut socket_addr_v6 = Default::default();
        let meta = connection_meta(
            fla.control_permissions.clone().into_option(),
            fla.controlled_context.clone().into_option(),
        );
        if peer_addr_v6.port() > 0 && !relay {
            socket_addr_v6 = start_ipv6(peer_addr_v6, addr, server.clone(), meta.clone()).await;
        }
        if is_ipv4(&self.addr) && !relay && !config::is_disable_tcp_listen() {
            if let Err(err) = self
                .handle_intranet_(
                    fla.clone(),
                    server.clone(),
                    relay_server.clone(),
                    socket_addr_v6.clone(),
                    meta.clone(),
                )
                .await
            {
                log::debug!("Failed to handle intranet: {:?}, will try relay", err);
            } else {
                return Ok(());
            }
        }
        let uuid = Uuid::new_v4().to_string();
        self.create_relay(
            fla.socket_addr.into(),
            relay_server,
            uuid,
            server,
            true,
            true,
            socket_addr_v6,
            String::new(),
            String::new(),
            meta,
        )
        .await
    }

    async fn handle_intranet_(
        &self,
        fla: FetchLocalAddr,
        server: ServerPtr,
        relay_server: String,
        socket_addr_v6: bytes::Bytes,
        meta: ConnectionMeta,
    ) -> ResultType<()> {
        let peer_addr = AddrMangle::decode(&fla.socket_addr);
        log::debug!("Handle intranet from {:?}", peer_addr);
        let mut socket = connect_tcp(&*self.host, CONNECT_TIMEOUT).await?;
        let local_addr = socket.local_addr();
        // we saw invalid local_addr while using proxy, local_addr.ip() == "::1"
        let local_addr: SocketAddr =
            format!("{}:{}", local_addr.ip(), local_addr.port()).parse()?;
        let mut msg_out = Message::new();
        msg_out.set_local_addr(LocalAddr {
            id: Config::get_id(),
            socket_addr: AddrMangle::encode(peer_addr).into(),
            local_addr: AddrMangle::encode(local_addr).into(),
            relay_server,
            version: crate::VERSION.to_owned(),
            socket_addr_v6,
            ..Default::default()
        });
        let bytes = msg_out.write_to_bytes()?;
        socket.send_raw(bytes).await?;
        crate::accept_connection(server.clone(), socket, peer_addr, true, meta).await;
        Ok(())
    }
}

const PEER_TICKET_WAIT_MS: u64 = 1_500;

/// Only an unattended peer (no session of its own) that initiated the relay waits for hbbs to
/// answer its RelayResponse with a ticket (openuu-server docs/relay-ticket-peer-initiated.md).
/// A logged-in peer, or one that already holds a forwarded ticket, keeps its timing untouched.
fn waits_for_peer_ticket(initiate: bool, peer_ticket: &str) -> bool {
    initiate && peer_ticket.is_empty() && crate::account::session_token().is_empty()
}

/// The ticket a new hbbs sends back as RequestRelay{uuid, token} on the socket the
/// RelayResponse went out on; empty when nothing matching arrives in time (old hbbs).
async fn peer_ticket_from_hbbs(socket: &mut Stream, uuid: &str) -> String {
    if let Some(Ok(bytes)) = socket.next_timeout(PEER_TICKET_WAIT_MS).await {
        if let Ok(msg) = RendezvousMessage::parse_from_bytes(&bytes) {
            if let Some(rendezvous_message::Union::RequestRelay(rf)) = msg.union {
                if rf.uuid == uuid && !rf.token.is_empty() {
                    log::info!("peer ticket received from hbbs for relay {}", uuid);
                    return rf.token;
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::net::{TcpListener, TcpStream};

    #[test]
    fn only_an_unattended_initiator_waits() {
        // No account is configured in the test environment: session_token() is empty.
        assert!(waits_for_peer_ticket(true, ""));
        assert!(!waits_for_peer_ticket(false, ""), "answering a controller's request");
        assert!(!waits_for_peer_ticket(true, "ticket"), "already holds a forwarded ticket");
    }

    async fn pair() -> (Stream, Stream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (Stream::from(client, addr), Stream::from(server, addr))
    }

    fn request_relay(uuid: &str, token: &str) -> RendezvousMessage {
        let mut msg = RendezvousMessage::new();
        msg.set_request_relay(RequestRelay {
            uuid: uuid.into(),
            token: token.into(),
            ..Default::default()
        });
        msg
    }

    #[hbb_common::tokio::test]
    async fn takes_the_ticket_for_its_own_uuid() {
        let (mut peer, mut hbbs) = pair().await;
        hbbs.send(&request_relay("uuid-1", "ticket-1")).await.unwrap();
        assert_eq!(peer_ticket_from_hbbs(&mut peer, "uuid-1").await, "ticket-1");
    }

    #[hbb_common::tokio::test]
    async fn ignores_another_uuid_and_silence() {
        let (mut peer, mut hbbs) = pair().await;
        hbbs.send(&request_relay("uuid-2", "ticket-2")).await.unwrap();
        assert_eq!(peer_ticket_from_hbbs(&mut peer, "uuid-1").await, "");
        let (mut peer, _hbbs) = pair().await;
        let started = std::time::Instant::now();
        assert_eq!(peer_ticket_from_hbbs(&mut peer, "uuid-1").await, "");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
    }
}
