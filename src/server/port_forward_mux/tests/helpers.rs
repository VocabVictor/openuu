use super::*;

pub(super) fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// An echo server standing in for the forward target.
pub(super) async fn echo_target() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                loop {
                    let n = s.read(&mut buf).await.unwrap_or(0);
                    if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

pub(super) fn open(id: i32, port: u16) -> PortForwardChannel {
    let mut ch = PortForwardChannel::new();
    ch.set_open(PortForwardOpen {
        channel_id: id,
        host: "127.0.0.1".to_owned(),
        port: port as i32,
        window: CHANNEL_WINDOW,
        ..Default::default()
    });
    ch
}

pub(super) fn data(id: i32, bytes: &[u8]) -> PortForwardChannel {
    let mut ch = PortForwardChannel::new();
    ch.set_data(PortForwardData {
        channel_id: id,
        data: Bytes::copy_from_slice(bytes),
        ..Default::default()
    });
    ch
}

pub(super) fn close(id: i32) -> PortForwardChannel {
    let mut ch = PortForwardChannel::new();
    ch.set_close(PortForwardClose { channel_id: id, ..Default::default() });
    ch
}

pub(super) async fn next_frame(rx: &mut mpsc::UnboundedReceiver<(Instant, Arc<Message>)>) -> PortForwardChannel {
    let (_, m) = rx.recv().await.unwrap();
    match &m.union {
        Some(message::Union::PortForwardChannel(ch)) => ch.clone(),
        other => panic!("unexpected {:?}", other),
    }
}

pub(super) fn opened(ch: &PortForwardChannel) -> (i32, bool) {
    match &ch.union {
        Some(port_forward_channel::Union::Opened(o)) => (o.channel_id, o.success),
        other => panic!("expected opened, got {:?}", other),
    }
}

pub(super) fn data_of(ch: &PortForwardChannel) -> (i32, Vec<u8>) {
    match &ch.union {
        Some(port_forward_channel::Union::Data(d)) => (d.channel_id, d.data.to_vec()),
        other => panic!("expected data, got {:?}", other),
    }
}
