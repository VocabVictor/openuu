use super::*;

/// Where a channel's frames go. The controller keeps two queues so
/// `window_update` can bypass bulk data; the controlled side has the
/// connection's single ordered `inner.tx`. `open` is not control: it rides
/// the ordered queue so it can never arrive after the channel's first `data`.
#[derive(Clone)]
pub enum FrameSink {
    Queued {
        data: mpsc::Sender<Message>,
        control: mpsc::UnboundedSender<Message>,
    },
    Direct(mpsc::UnboundedSender<(Instant, Arc<Message>)>),
}

fn writer_gone(what: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, format!("{} gone", what))
}

impl FrameSink {
    pub async fn send_ordered(&self, msg: Message) -> ResultType<()> {
        match self {
            FrameSink::Queued { data, .. } => data
                .send(msg)
                .await
                .map_err(|_| writer_gone("tunnel writer").into()),
            FrameSink::Direct(tx) => tx
                .send((Instant::now(), Arc::new(msg)))
                .map_err(|_| writer_gone("connection writer").into()),
        }
    }

    pub fn send_control(&self, msg: Message) -> ResultType<()> {
        match self {
            FrameSink::Queued { control, .. } => control
                .send(msg)
                .map_err(|_| writer_gone("tunnel writer").into()),
            FrameSink::Direct(tx) => tx
                .send((Instant::now(), Arc::new(msg)))
                .map_err(|_| writer_gone("connection writer").into()),
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            FrameSink::Queued { data, .. } => data.is_closed(),
            FrameSink::Direct(tx) => tx.is_closed(),
        }
    }
}

pub enum Inbound {
    Data(Bytes),
    Close,
    /// The demultiplexer found the peer over its window; the channel task
    /// closes and tells the peer, so the frame still leaves in order.
    Violation,
}

#[derive(Debug, PartialEq)]
pub enum RelayEnd {
    LocalEof,
    PeerClosed,
    Violation,
    Cancelled,
    TunnelGone,
}

/// Local socket -> tunnel, under the peer's credit. `prebuf` is simply the
/// head of the byte stream.
async fn relay_socket_to_tunnel<R: AsyncRead + Unpin>(
    id: i32,
    reader: R,
    prebuf: Vec<u8>,
    credit: Arc<SendCredit>,
    sink: FrameSink,
    mut cancel: watch::Receiver<bool>,
) -> RelayEnd {
    let mut reader = std::io::Cursor::new(prebuf).chain(reader);
    // One scratch buffer per channel and an exact-size copy per frame: a frame
    // that owned its read allocation would pin up to MAX_FRAME until sent,
    // whatever its length, and interactive traffic is mostly tiny frames.
    let mut scratch = vec![0u8; MAX_FRAME];
    loop {
        let allow = tokio::select! {
            n = credit.take(MAX_FRAME) => n,
            _ = cancel.changed() => return RelayEnd::Cancelled,
        };
        let got = tokio::select! {
            r = reader.read(&mut scratch[..allow]) => r.unwrap_or(0),
            _ = cancel.changed() => {
                credit.add(allow as u32);
                return RelayEnd::Cancelled;
            }
        };
        let spent = if got == 0 { 0 } else { charge(got) };
        if (spent as usize) < allow {
            credit.add(allow as u32 - spent);
        }
        if got == 0 {
            return RelayEnd::LocalEof;
        }
        let frame = data_msg(id, Bytes::copy_from_slice(&scratch[..got]));
        if sink.send_ordered(frame).await.is_err() {
            return RelayEnd::TunnelGone;
        }
    }
}

/// Tunnel -> local socket. `initial` is written before anything from the
/// queue (the controlled side's bytes buffered while connecting).
async fn relay_tunnel_to_socket<W: AsyncWrite + Unpin>(
    id: i32,
    mut writer: W,
    initial: Vec<Bytes>,
    mut inbound: mpsc::UnboundedReceiver<Inbound>,
    window: Arc<Mutex<RecvWindow>>,
    sink: FrameSink,
    mut cancel: watch::Receiver<bool>,
) -> RelayEnd {
    let mut pending: std::collections::VecDeque<Bytes> = initial.into();
    loop {
        let chunk = match pending.pop_front() {
            Some(c) => c,
            None => {
                let next = tokio::select! {
                    n = inbound.recv() => n,
                    _ = cancel.changed() => return RelayEnd::Cancelled,
                };
                match next {
                    Some(Inbound::Data(c)) => c,
                    Some(Inbound::Close) => return RelayEnd::PeerClosed,
                    Some(Inbound::Violation) => return RelayEnd::Violation,
                    None => return RelayEnd::TunnelGone,
                }
            }
        };
        let written = tokio::select! {
            r = writer.write_all(&chunk) => r.is_ok(),
            _ = cancel.changed() => return RelayEnd::Cancelled,
        };
        if !written {
            return RelayEnd::LocalEof;
        }
        let update = window.lock().unwrap().drained(chunk.len());
        if let Some(add) = update {
            if sink.send_control(window_update_msg(id, add)).is_err() {
                return RelayEnd::TunnelGone;
            }
        }
    }
}

/// Runs both halves as independent tasks; whichever ends first cancels the
/// other. Sends `close` once, after the last data, and only when the channel
/// ended for a local reason — the peer's own `close` is never echoed.
/// `teardown` is the tunnel closing under the channel: it cancels both halves
/// even when they are parked on the socket, where dropping the inbound sender
/// reaches neither. It is a level, so a channel opened as the tunnel closes,
/// subscribing after the signal went out, still sees it.
pub async fn run_channel<R, W>(
    id: i32,
    reader: R,
    writer: W,
    prebuf: Vec<u8>,
    initial_out: Vec<Bytes>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
    inbound: mpsc::UnboundedReceiver<Inbound>,
    sink: FrameSink,
    mut teardown: watch::Receiver<bool>,
) where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let mut to_tunnel = tokio::spawn(relay_socket_to_tunnel(
        id, reader, prebuf, credit, sink.clone(), cancel_rx.clone(),
    ));
    let mut to_socket = tokio::spawn(relay_tunnel_to_socket(
        id, writer, initial_out, inbound, window, sink.clone(), cancel_rx,
    ));
    let (first, second) = tokio::select! {
        r = &mut to_tunnel => {
            let _ = cancel_tx.send(true);
            (r.unwrap_or(RelayEnd::Cancelled), to_socket.await.unwrap_or(RelayEnd::Cancelled))
        }
        r = &mut to_socket => {
            let _ = cancel_tx.send(true);
            (r.unwrap_or(RelayEnd::Cancelled), to_tunnel.await.unwrap_or(RelayEnd::Cancelled))
        }
        // Wrapped so `select!` keeps a `bool`, not the `Ref` (a read guard,
        // not `Send`) it would otherwise hold across the joins.
        _ = async { teardown.wait_for(|down| *down).await.is_ok() } => {
            let _ = cancel_tx.send(true);
            (to_tunnel.await.unwrap_or(RelayEnd::Cancelled), to_socket.await.unwrap_or(RelayEnd::Cancelled))
        }
    };
    let peer_closed = first == RelayEnd::PeerClosed || second == RelayEnd::PeerClosed;
    let tunnel_gone = first == RelayEnd::TunnelGone || second == RelayEnd::TunnelGone;
    let local_reason = matches!(first, RelayEnd::LocalEof | RelayEnd::Violation)
        || matches!(second, RelayEnd::LocalEof | RelayEnd::Violation);
    if !peer_closed && !tunnel_gone && local_reason {
        if let Err(e) = sink.send_ordered(close_msg(id)).await {
            log::debug!("port forward channel {} close not sent: {}", id, e);
        }
    }
    log::debug!("port forward channel {} ended: {:?} / {:?}", id, first, second);
}
