use super::*;
use hbb_common::tokio::net::{TcpListener, TcpStream};

/// An `Interface` that answers nothing; the relay stage only reads its policy flags.
#[derive(Clone, Default)]
pub(super) struct NoUi;

#[async_trait]
impl Interface for NoUi {
    fn send(&self, _data: Data) {}
    fn msgbox(&self, _msgtype: &str, _title: &str, _text: &str, _link: &str) {}
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
    async fn handle_test_delay(&self, _t: TestDelay, _peer: &mut Stream) {}
    fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>> {
        Arc::new(RwLock::new(Default::default()))
    }
}

/// A loopback pair standing in for the rendezvous connection; the far end is kept so
/// the socket stays open for the duration of the call.
pub(super) async fn rendezvous_pair() -> (Stream, Stream, SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let client = TcpStream::connect(addr).await.unwrap();
    let my_addr = client.local_addr().unwrap();
    let (server_side, peer_addr) = listener.accept().await.unwrap();
    (
        Stream::from(client, addr),
        Stream::from(server_side, peer_addr),
        my_addr,
    )
}

fn relay_response(relay_server: &str) -> RelayResponse {
    RelayResponse {
        uuid: "u-1".to_owned(),
        relay_server: relay_server.to_owned(),
        feedback: 3,
        ..Default::default()
    }
}

#[tokio::test]
async fn an_unreachable_relay_fails_the_relay_stage() {
    let (socket, _far_end, my_addr) = rendezvous_pair().await;
    let ui = NoUi;
    let ctx = StartCtx {
        peer: "123456789",
        key: "",
        token: "",
        conn_type: ConnType::DEFAULT_CONN,
        interface: ui,
        rendezvous_server: "127.0.0.1:1",
        my_addr,
        start: Instant::now(),
    };
    // Port 1 on loopback refuses at once, so the relay race has no winner.
    let result = Client::connect_on_relay_response(
        relay_response("127.0.0.1:1"),
        socket,
        None,
        None,
        Vec::new(),
        ctx,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn a_webrtc_answer_without_an_offerer_falls_back_to_the_relay_race() {
    let (socket, _far_end, my_addr) = rendezvous_pair().await;
    let ctx = StartCtx {
        peer: "123456789",
        key: "",
        token: "",
        conn_type: ConnType::DEFAULT_CONN,
        interface: NoUi,
        rendezvous_server: "127.0.0.1:1",
        my_addr,
        start: Instant::now(),
    };
    let mut rr = relay_response("127.0.0.1:1");
    rr.webrtc_sdp_answer = "v=0".to_owned();
    let result = Client::connect_on_relay_response(
        rr,
        socket,
        None,
        None,
        vec!["candidate:1".to_owned()],
        ctx,
    )
    .await;
    assert!(result.is_err());
}
