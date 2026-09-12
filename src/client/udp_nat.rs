use super::*;

pub(super) async fn test_udp_uat(
    udp_socket: Arc<UdpSocket>,
    server_addr: SocketAddr,
    udp_port: Arc<Mutex<u16>>,
    mut stop_udp_rx: oneshot::Receiver<()>,
) -> ResultType<()> {
    // The punch port must come only from the rendezvous server's TestNatResponse, which
    // observes THIS socket's public mapping. A STUN probe binds a different socket and reports
    // a different NAT mapping, so racing it here could advertise a port the peer can never
    // reach (and, on symmetric NAT, silently poison the whole UDP punch).
    let start = Instant::now();
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_test_nat_request(TestNatRequest {
        ..Default::default()
    });
    // Adaptive retry strategy that works within TCP RTT constraints
    // Start with aggressive sending, then back off
    let mut retry_interval = Duration::from_millis(20); // Start fast
    pub(super) const MAX_INTERVAL: Duration = Duration::from_millis(200);
    let mut packets_sent = 0;

    // Send initial burst to improve reliability
    let data = msg_out.write_to_bytes()?;
    for _ in 0..2 {
        if let Err(e) = udp_socket.send_to(&data, server_addr).await {
            log::warn!("Failed to send initial UDP NAT test packet: {}", e);
        } else {
            packets_sent += 1;
        }
    }
    let mut last_send_time = Instant::now();
    let mut buf = [0u8; 1500];

    loop {
        tokio::select! {
            _ = &mut stop_udp_rx => {
                log::debug!("UDP NAT test received stop signal after {} packets", packets_sent);
                break;
            }
            _ = hbb_common::sleep(retry_interval.as_secs_f32()) => {
                // Adaptive retry: send fewer packets as time goes on
                let elapsed = last_send_time.elapsed();

                if elapsed >= retry_interval {
                    // Send single packet (not double) to reduce network load
                    if let Err(e) = udp_socket.send_to(&data, server_addr).await {
                        log::warn!("Failed to send UDP NAT test retry packet: {}", e);
                    } else {
                        packets_sent += 1;
                    }

                    // Exponentially increase interval to reduce network pressure
                    retry_interval = std::cmp::min(
                        Duration::from_millis((retry_interval.as_millis() as f64 * 1.5) as u64),
                        MAX_INTERVAL
                    );
                    last_send_time = Instant::now();
                }
            }
            res = udp_socket.recv(&mut buf[..]) => {
                match res {
                    Ok(n) => {
                        match RendezvousMessage::parse_from_bytes(&buf[0..n]) {
                            Ok(msg_in) => {
                                if let Some(rendezvous_message::Union::TestNatResponse(response)) = msg_in.union {
                                    *udp_port.lock().unwrap() = response.port as u16;
                                    break;
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to parse UDP NAT test response: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        // Same ICMP-driven errors as punch_udp sees. Without a pause this arm
                        // re-arms recv immediately and spins the loop at CPU speed.
                        if let Some(n) = UDP_UAT_ERR_LOG.due() {
                            log::warn!("UDP NAT test socket error x{n}, last: {e}");
                        }
                        hbb_common::sleep(0.01).await;
                    }
                }
            }
        }
    }

    let final_port = *udp_port.lock().unwrap();
    log::debug!(
        "UDP NAT test to {:?} finished: time={:?}, port={}, packets_sent={}, success={}",
        server_addr,
        start.elapsed(),
        final_port,
        packets_sent,
        final_port > 0
    );
    Ok(())
}

#[inline]
pub(super) async fn udp_nat_connect(
    socket: Arc<UdpSocket>,
    typ: &'static str,
    ms_timeout: u64,
) -> ResultType<(Stream, Option<KcpStream>, &'static str)> {
    crate::punch_udp(socket.clone(), false)
        .await
        .map_err(|err| {
            log::debug!("{err}");
            anyhow!(err)
        })?;
    let res = KcpStream::connect(socket, Duration::from_millis(ms_timeout))
        .await
        .map_err(|err| {
            log::debug!("Failed to connect KCP stream: {}", err);
            anyhow!(err)
        })?;
    Ok((res.1, Some(res.0), typ))
}
