use super::*;

pub async fn hc_connection(
    feedback: i32,
    rendezvous_server: String,
    token: &str,
) -> Option<tokio::sync::mpsc::UnboundedSender<()>> {
    if feedback == 0 || rendezvous_server.is_empty() || token.is_empty() {
        return None;
    }
    let (tx, rx) = unbounded_channel::<()>();
    let token = token.to_owned();
    tokio::spawn(async move {
        allow_err!(hc_connection_(rendezvous_server, rx, token).await);
    });
    Some(tx)
}

pub(super) async fn hc_connection_(
    rendezvous_server: String,
    mut rx: UnboundedReceiver<()>,
    token: String,
) -> ResultType<()> {
    let mut timer = crate::rustdesk_interval(interval(crate::TIMER_OUT));
    let mut last_recv_msg = Instant::now();
    let mut keep_alive = crate::DEFAULT_KEEP_ALIVE;

    let host = check_port(&rendezvous_server, RENDEZVOUS_PORT);
    let mut conn = connect_tcp(host.clone(), CONNECT_TIMEOUT).await?;
    let key = crate::get_key(true).await;
    crate::secure_tcp(&mut conn, &key).await?;
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_hc(HealthCheck {
        token,
        ..Default::default()
    });
    conn.send(&msg_out).await?;
    loop {
        tokio::select! {
            res = rx.recv() => {
                if res.is_none() {
                    log::debug!("HC connection is closed as controlling connection exits");
                    break;
                }
            }
            res = conn.next() => {
                last_recv_msg = Instant::now();
                let bytes = res.ok_or_else(|| anyhow!("Rendezvous connection is reset by the peer"))??;
                if bytes.is_empty() {
                    conn.send_bytes(bytes::Bytes::new()).await?;
                    continue; // heartbeat
                }
                let msg = RendezvousMessage::parse_from_bytes(&bytes)?;
                match msg.union {
                    Some(rendezvous_message::Union::RegisterPkResponse(rpr)) => {
                        if rpr.keep_alive > 0 {
                            keep_alive = rpr.keep_alive * 1000;
                            log::info!("keep_alive: {}ms", keep_alive);
                        }
                    }
                    _ => {}
                }
            }
            _  = timer.tick() => {
                // https://www.emqx.com/en/blog/mqtt-keep-alive
                if last_recv_msg.elapsed().as_millis() as u64 > keep_alive as u64 * 3 / 2 {
                    bail!("HC connection is timeout");
                }
            }
        }
    }
    Ok(())
}
