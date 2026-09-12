use super::*;

mod muxed;
mod open;
mod state;
use crate::port_forward_mux::{tunnel::{TunnelHandle, MAX_REPORTED_OPEN_ERRORS}, Claim, Tunnel};
use hbb_common::{
    protobuf::Message as _,
    tcp::FramedStream,
    tokio::net::{TcpListener, TcpStream},
    Stream,
};

/// A loopback TCP pair wrapped as two `Stream`s: one for the tunnel,
/// one for the fake peer.
async fn stream_pair() -> (Stream, Stream) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let client = TcpStream::connect(addr).await.unwrap();
    let (server, _) = l.accept().await.unwrap();
    (
        Stream::Tcp(FramedStream::from(client, addr)),
        Stream::Tcp(FramedStream::from(server, addr)),
    )
}

async fn recv_frame(s: &mut Stream) -> PortForwardChannel {
    let bytes = s.next().await.unwrap().unwrap();
    let m = Message::parse_from_bytes(&bytes).unwrap();
    match m.union {
        Some(message::Union::PortForwardChannel(ch)) => ch,
        other => panic!("unexpected {:?}", other),
    }
}

async fn local_pair() -> (TcpStream, TcpStream) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let a = TcpStream::connect(addr).await.unwrap();
    let (b, _) = l.accept().await.unwrap();
    (a, b)
}

/// An `Interface` that records the dialogs it was asked to show. The
/// tunnel needs it for `handle_test_delay` and for refusal messages.
#[derive(Clone, Default)]
pub struct NoUi(Arc<Mutex<Vec<String>>>);

impl NoUi {
    fn messages(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl crate::client::Interface for NoUi {
    fn send(&self, _data: crate::client::Data) {}
    fn msgbox(&self, _msgtype: &str, _title: &str, text: &str, _link: &str) {
        self.0.lock().unwrap().push(text.to_owned());
    }
    fn handle_login_error(&self, _err: &str) -> bool {
        false
    }
    fn handle_peer_info(&self, _pi: PeerInfo) {}
    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
    async fn handle_hash(&self, _pass: &str, _hash: Hash, _peer: &mut Stream) -> bool {
        false
    }
    async fn handle_login_from_ui(
        &self,
        _os_username: String,
        _os_password: String,
        _password: String,
        _remember: bool,
        _peer: &mut Stream,
    ) {
    }
    async fn handle_test_delay(&self, t: TestDelay, peer: &mut Stream) {
        if !t.from_client {
            crate::client::handle_test_delay(t, peer).await;
        }
    }
    fn get_lch(&self) -> Arc<std::sync::RwLock<crate::client::LoginConfigHandler>> {
        Arc::new(std::sync::RwLock::new(Default::default()))
    }
}

use crate::server::port_forward_mux::PortForwardMux;
use hbb_common::tokio::time::Instant;

/// Stands in for `Connection`: one task owning the stream, draining
/// `inner.tx` into it and dispatching inbound frames to the mux.
fn fake_controlled(mut stream: Stream, login_target: String) {
    tokio::spawn(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let mut mux = PortForwardMux::new(tx, login_target);
        loop {
            tokio::select! {
                Some((_, m)) = rx.recv() => {
                    if stream.send(&*m).await.is_err() { return; }
                }
                res = stream.next() => match res {
                    Some(Ok(bytes)) => {
                        let Ok(m) = Message::parse_from_bytes(&bytes) else { continue };
                        if let Some(message::Union::PortForwardChannel(ch)) = m.union {
                            mux.handle(ch, || true);
                        }
                    }
                    _ => return,
                },
            }
        }
    });
}

async fn echo_target() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let n = s.read(&mut buf).await.unwrap_or(0);
                    if n == 0 || s.write_all(&buf[..n]).await.is_err() { return; }
                }
            });
        }
    });
    port
}

/// The `Tunnel` comes back too: dropping it is what ends the peer, so
/// a test that wants a live tunnel has to keep holding it.
async fn muxed_tunnel() -> (Tunnel, Arc<TunnelHandle>, u16) {
    let (ours, theirs) = stream_pair().await;
    let port = echo_target().await;
    fake_controlled(theirs, format!("127.0.0.1:{}", port));
    let t = Tunnel::new();
    t.claim();
    let h = t.set_muxed(ours, NoUi::default());
    (t, h, port)
}
