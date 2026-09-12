use super::*;

fn get_direct_port() -> i32 {
    let mut port = Config::get_option("direct-access-port")
        .parse::<i32>()
        .unwrap_or(0);
    if port <= 0 {
        port = RENDEZVOUS_PORT + 2;
    }
    port
}

pub(super) async fn direct_server(server: ServerPtr) {
    let mut listener = None;
    let mut port = 0;
    loop {
        let disabled = !option2bool(
            OPTION_DIRECT_SERVER,
            &Config::get_option(OPTION_DIRECT_SERVER),
        ) || option2bool("stop-service", &Config::get_option("stop-service"));
        if !disabled && listener.is_none() {
            port = get_direct_port();
            match hbb_common::tcp::listen_any(port as _).await {
                Ok(l) => {
                    listener = Some(l);
                    log::info!(
                        "Direct server listening on: {:?}",
                        listener.as_ref().map(|l| l.local_addr())
                    );
                }
                Err(err) => {
                    // to-do: pass to ui
                    log::error!(
                        "Failed to start direct server on port: {}, error: {}",
                        port,
                        err
                    );
                    loop {
                        if port != get_direct_port() {
                            break;
                        }
                        sleep(1.).await;
                    }
                }
            }
        }
        if let Some(l) = listener.as_mut() {
            if disabled || port != get_direct_port() {
                log::info!("Exit direct access listen");
                listener = None;
                continue;
            }
            if let Ok(Ok((stream, addr))) = hbb_common::timeout(1000, l.accept()).await {
                stream.set_nodelay(true).ok();
                log::info!("direct access from {}", addr);
                let local_addr = stream
                    .local_addr()
                    .unwrap_or(Config::get_any_listen_addr(true));
                let server = server.clone();
                tokio::spawn(async move {
                    allow_err!(
                        crate::server::create_tcp_connection(
                            server,
                            hbb_common::Stream::from(stream, local_addr),
                            addr,
                            false,
                            ConnectionMeta::default(), // Direct connections don't have server-side user context.
                        )
                        .await
                    );
                });
            } else {
                sleep(0.1).await;
            }
        } else {
            sleep(1.).await;
        }
    }
}

pub(super) async fn start_ipv6(
    peer_addr_v6: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
    meta: ConnectionMeta,
) -> bytes::Bytes {
    crate::test_ipv6().await;
    if let Some((socket, local_addr_v6)) = crate::get_ipv6_socket().await {
        let server = server.clone();
        tokio::spawn(async move {
            allow_err!(
                udp_nat_listen(socket.clone(), peer_addr_v6, peer_addr_v4, server, meta).await
            );
        });
        return local_addr_v6;
    }
    Default::default()
}

pub(super) async fn udp_nat_listen(
    socket: Arc<tokio::net::UdpSocket>,
    peer_addr: SocketAddr,
    peer_addr_v4: SocketAddr,
    server: ServerPtr,
    meta: ConnectionMeta,
) -> ResultType<()> {
    let tm = Instant::now();
    let socket_cloned = socket.clone();
    let func = async {
        socket.connect(peer_addr).await?;
        let init_packet = crate::punch_udp(socket.clone(), true).await?;
        let stream = crate::kcp_stream::KcpStream::accept(
            socket,
            Duration::from_millis(CONNECT_TIMEOUT as _),
            init_packet,
        )
        .await?;
        crate::server::create_tcp_connection(server, stream.1, peer_addr_v4, true, meta).await?;
        Ok(())
    };
    func.await.map_err(|e: anyhow::Error| {
        anyhow::anyhow!(
            "Stop listening on {:?} for remote {peer_addr} with KCP, {:?} elapsed: {e}",
            socket_cloned.local_addr(),
            tm.elapsed()
        )
    })?;
    Ok(())
}
