use super::*;

/// Owns the whole channel lifecycle: connect under a `select!` in which a
/// queued command always wins over the connect, buffer what arrives
/// meanwhile, then relay.
pub(super) async fn run_controlled_channel(
    id: i32,
    addr: String,
    is_rdp: bool,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    sink: FrameSink,
    teardown: watch::Receiver<bool>,
) {
    let mut pending: Vec<Bytes> = Vec::new();
    let mut pending_len = 0usize;
    let connect = timeout(CONNECT_TIMEOUT_MS, TcpStream::connect(&addr));
    tokio::pin!(connect);
    let socket = loop {
        tokio::select! {
            // Biased with the command arm first: a `close` that is already
            // queued must win over a connect that completed on the same poll,
            // or `opened` would go out for a channel the controller has dropped.
            biased;
            cmd = inbound.recv() => match cmd {
                Some(Inbound::Data(b)) => {
                    if !pending_fits(pending_len, b.len()) {
                        log::warn!("port forward channel {} sent more than INITIAL_WINDOW before opened", id);
                        sink.send_ordered(close_msg(id)).await.ok();
                        return;
                    }
                    pending_len += charge(b.len()) as usize;
                    pending.push(b);
                }
                Some(Inbound::Close) | None => return,
                Some(Inbound::Violation) => {
                    sink.send_ordered(close_msg(id)).await.ok();
                    return;
                }
            },
            res = &mut connect => {
                let err = match res {
                    Ok(Ok(s)) => break s,
                    Ok(Err(e)) => e.to_string(),
                    Err(e) => e.to_string(),
                };
                log::debug!("port forward channel {} connect {} failed: {}", id, addr, err);
                sink.send_ordered(opened_msg(id, false, &unreachable_message(&addr, is_rdp), 0)).await.ok();
                return;
            }
        }
    };
    // Granted before `opened` leaves, so the peer can never be ahead of it.
    window.lock().unwrap().grant(CHANNEL_WINDOW - INITIAL_WINDOW);
    if sink
        .send_ordered(opened_msg(id, true, "", CHANNEL_WINDOW))
        .await
        .is_err()
    {
        return;
    }
    let (reader, writer) = socket.into_split();
    run_channel(id, reader, writer, Vec::new(), pending, credit, window, inbound, sink, teardown).await;
}

/// The same words the raw pipe puts in its login error, so one problem reads
/// the same whichever path the peer takes.
pub(super) fn unreachable_message(addr: &str, is_rdp: bool) -> String {
    format!(
        "Failed to access remote {}. Please make sure it is reachable/open.",
        if is_rdp { "RDP" } else { addr }
    )
}
