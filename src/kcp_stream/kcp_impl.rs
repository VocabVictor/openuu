use super::*;

impl KcpStream {
    // Opt in to KCP's built-in congestion window (nc=0) instead of the pure turbo profile
    // (nc=1) that has always shipped; see `get_kcp_cc_enabled` for why this is not the default.
    // Sender-side only, so no wire negotiation is needed and either peer may run either profile.
    // Requires kcp-sys from the `rustdesk-patches` branch, which wires the config factory into
    // connection setup (on older revs the factory was stored but never consulted).
    pub(super) fn apply_kcp_config(endpoint: &mut KcpEndpoint) {
        if crate::get_kcp_cc_enabled() {
            endpoint.set_kcp_config_factory(Box::new(|conv| {
                let mut config = kcp_sys::ffi_safe::KcpConfig::new_turbo(conv);
                config.nc = Some(0);
                config
            }));
        }
    }

    /// How long since a valid packet was last received from the peer, or `None` once the
    /// connection is gone. Answered by the KCP endpoint's own tasks, not by the session's read
    /// loop, so it stays meaningful while that loop is busy sending a large message; and the
    /// endpoint pings an idle peer often enough that silence here means the peer, not quiet.
    pub fn peer_silent_for(&self) -> Option<std::time::Duration> {
        self.endpoint.peer_silent_for(&self.conn_id)
    }

    pub(super) fn create_framed(stream: stream::KcpStream, local_addr: Option<SocketAddr>) -> Stream {
        Stream::Tcp(FramedStream(
            tokio_util::codec::Framed::new(DynTcpStream(Box::new(stream)), BytesCodec::new()),
            local_addr.unwrap_or(config::Config::get_any_listen_addr(true)),
            None,
            0,
        ))
    }

    pub async fn accept(
        udp_socket: Arc<UdpSocket>,
        timeout: std::time::Duration,
        init_packet: Option<BytesMut>,
    ) -> ResultType<(Self, Stream)> {
        let mut endpoint = KcpEndpoint::new();
        Self::apply_kcp_config(&mut endpoint);
        endpoint.run().await;

        let (input, output) = (
            endpoint.input_sender(),
            endpoint
                .output_receiver()
                .ok_or_else(|| anyhow::anyhow!("Failed to get output receiver"))?,
        );
        let (stop_sender, stop_receiver) = oneshot::channel();
        if let Some(packet) = init_packet {
            if packet.len() >= std::mem::size_of::<KcpPacketHeader>() {
                input.send(packet.into()).await?;
            }
        }
        Self::kcp_io(udp_socket.clone(), input, output, stop_receiver).await;

        let conn_id = tokio::time::timeout(timeout, endpoint.accept()).await??;
        if let Some(stream) = stream::KcpStream::new(&endpoint, conn_id) {
            Ok((
                Self {
                    endpoint,
                    conn_id,
                    stop_sender: Some(stop_sender),
                },
                Self::create_framed(stream, udp_socket.local_addr().ok()),
            ))
        } else {
            Err(anyhow::anyhow!("Failed to create KcpStream"))
        }
    }

    pub async fn connect(
        udp_socket: Arc<UdpSocket>,
        timeout: std::time::Duration,
    ) -> ResultType<(Self, Stream)> {
        let mut endpoint = KcpEndpoint::new();
        Self::apply_kcp_config(&mut endpoint);
        endpoint.run().await;

        let (input, output) = (
            endpoint.input_sender(),
            endpoint
                .output_receiver()
                .ok_or_else(|| anyhow::anyhow!("Failed to get output receiver"))?,
        );
        let (stop_sender, stop_receiver) = oneshot::channel();
        Self::kcp_io(udp_socket.clone(), input, output, stop_receiver).await;

        let conn_id = endpoint.connect(timeout, 0, 0, Bytes::new()).await?;
        if let Some(stream) = stream::KcpStream::new(&endpoint, conn_id) {
            Ok((
                Self {
                    endpoint,
                    conn_id,
                    stop_sender: Some(stop_sender),
                },
                Self::create_framed(stream, udp_socket.local_addr().ok()),
            ))
        } else {
            Err(anyhow::anyhow!("Failed to create KcpStream"))
        }
    }

    pub(super) async fn kcp_io(
        udp_socket: Arc<UdpSocket>,
        input: mpsc::Sender<KcpPacket>,
        mut output: mpsc::Receiver<KcpPacket>,
        mut stop_receiver: oneshot::Receiver<()>,
    ) {
        let udp = udp_socket.clone();
        tokio::spawn(async move {
            let mut buf = vec![0; 1500];
            // Socket errors are ICMP unreachable on a connected UDP socket — advisory, and
            // routine while a hole forms — so treat them as loss and let KCP's pong timeout reap
            // a link that is really dead. One throttle PER DIRECTION: the error is reported once
            // and cleared, so send-ok/recv-err alternates and a shared counter never fires.
            loop {
                tokio::select! {
                    _ = &mut stop_receiver => {
                        log::debug!("KCP io loop received stop signal");
                        break;
                    }
                    Some(data) = output.recv() => {
                        if let Err(e) = udp.send(&data.inner()).await {
                            if let Some(n) = KCP_SEND_ERR_LOG.due() {
                                log::debug!("KCP send error x{n} (treated as loss), last: {e}");
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    }
                    result = udp.recv_from(&mut buf) => {
                        match result {
                            Ok((size, _)) => {
                                if size < std::mem::size_of::<KcpPacketHeader>() {
                                    continue;
                                }
                                input
                                    .send(BytesMut::from(&buf[..size]).into())
                                    .await.ok();
                            }
                            Err(e) => {
                                if let Some(n) = KCP_RECV_ERR_LOG.due() {
                                    log::debug!("KCP recv error x{n} (treated as loss), last: {e}");
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                            }
                        }
                    }
                    else => {
                        log::debug!("KCP endpoint input closed");
                        break;
                    }
                }
            }
        });
    }
}

impl Drop for KcpStream {
    fn drop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }
}
