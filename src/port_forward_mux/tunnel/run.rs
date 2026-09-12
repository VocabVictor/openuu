use super::*;

/// The only task that touches the stream. The three arms keep tokio's
/// default random fairness: a `biased` control -> read -> data order would
/// starve outbound data whenever inbound is saturated (a LAN-speed
/// download keeps the read arm ready on every poll), and the reverse would
/// starve the reads that carry the peer's window updates and pings.
/// Random order is safe only because nothing order-sensitive is split
/// across the queues: `open`, `data` and `close` share the data queue and
/// the control queue carries `window_update` alone.
pub(super) async fn tunnel_loop(
    mut stream: Stream,
    handle: Arc<TunnelHandle>,
    mut data_rx: mpsc::Receiver<Message>,
    mut control_rx: mpsc::UnboundedReceiver<Message>,
    interface: impl Interface,
    state: watch::Sender<TunnelState>,
    mut lifetime: watch::Receiver<()>,
) {
    let mut account_timer = tokio::time::interval(std::time::Duration::from_secs(1));
    let err = loop {
        tokio::select! {
            _ = account_timer.tick() => {
                if crate::account::require_login().await.is_err() { break "OpenUU login expired".to_owned(); }
            }
            Some(msg) = control_rx.recv() => {
                if let Err(e) = stream.send(&msg).await {
                    break format!("send failed: {}", e);
                }
            }
            res = stream.next_timeout(READ_TIMEOUT) => match res {
                Some(Ok(bytes)) => {
                    let Ok(msg) = Message::parse_from_bytes(&bytes) else { continue };
                    match msg.union {
                        Some(message::Union::PortForwardChannel(ch)) => {
                            if let Some(err) = handle.on_frame(ch) {
                                interface.on_error(&err);
                            }
                        }
                        Some(message::Union::TestDelay(t)) => {
                            interface.handle_test_delay(t, &mut stream).await;
                        }
                        Some(message::Union::Misc(misc)) => {
                            if let Some(misc::Union::CloseReason(r)) = misc.union {
                                break format!("closed by peer: {}", r);
                            }
                        }
                        _ => {}
                    }
                }
                Some(Err(e)) => break format!("read failed: {}", e),
                None => break "timeout or reset by the peer".to_owned(),
            },
            Some(msg) = data_rx.recv() => {
                if let Err(e) = stream.send(&msg).await {
                    break format!("send failed: {}", e);
                }
            }
            _ = lifetime.changed() => break "window closed".to_owned(),
        }
    };
    log::info!("port forward tunnel ended: {}", err);
    handle.close_all();
    state.send_replace(TunnelState::Unset);
}
