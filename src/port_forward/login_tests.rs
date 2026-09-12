use super::*;
use async_trait::async_trait;
use hbb_common::{
    tcp::FramedStream,
    tokio::time::{sleep, Duration},
};
use sha2::{Digest, Sha256};

/// A window's interface over its shared handler. `handle_hash` can pause
/// before building the login, where the real one looks passwords up.
#[derive(Clone)]
struct Ui {
    lc: Arc<RwLock<LoginConfigHandler>>,
    pause: Duration,
}

#[async_trait]
impl Interface for Ui {
    fn send(&self, _data: Data) {}
    fn msgbox(&self, _msgtype: &str, _title: &str, _text: &str, _link: &str) {}
    fn handle_login_error(&self, _err: &str) -> bool {
        false
    }
    fn handle_peer_info(&self, _pi: PeerInfo) {}
    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}
    async fn handle_hash(&self, pass: &str, hash: Hash, peer: &mut Stream) -> bool {
        sleep(self.pause).await;
        crate::client::handle_hash(self.lc.clone(), pass, hash, self, peer).await
    }
    async fn handle_login_from_ui(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
        peer: &mut Stream,
    ) {
        crate::client::handle_login_from_ui(
            self.lc.clone(),
            os_username,
            os_password,
            password,
            remember,
            peer,
        )
        .await
    }
    async fn handle_test_delay(&self, _t: TestDelay, _peer: &mut Stream) {}
    fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>> {
        self.lc.clone()
    }
}

fn window() -> Ui {
    let mut lc = LoginConfigHandler::default();
    lc.conn_type = ConnType::PORT_FORWARD;
    Ui {
        lc: Arc::new(RwLock::new(lc)),
        pause: Duration::ZERO,
    }
}

/// (our end, the peer's end) of one connection.
async fn loopback() -> (Stream, Stream) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let client = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (server, _) = l.accept().await.unwrap();
    (
        Stream::Tcp(FramedStream::from(client, addr)),
        Stream::Tcp(FramedStream::from(server, addr)),
    )
}

async fn login_at(peer: &mut Stream) -> LoginRequest {
    let bytes = peer.next().await.unwrap().unwrap();
    Message::parse_from_bytes(&bytes)
        .unwrap()
        .login_request()
        .clone()
}

fn target(lr: &LoginRequest) -> (String, i32) {
    (lr.port_forward().host.clone(), lr.port_forward().port)
}

fn hash(challenge: &str) -> Hash {
    Hash {
        salt: "salt".to_owned(),
        challenge: challenge.to_owned(),
        ..Default::default()
    }
}

/// What the peer expects for password `pw` under `hash(challenge)`.
fn digest(challenge: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update("pw");
    h.update("salt");
    let salted = h.finalize();
    let mut h2 = Sha256::new();
    h2.update(&salted[..]);
    h2.update(challenge);
    h2.finalize()[..].to_vec()
}

#[test]
fn mappings_logging_in_at_once_each_carry_their_own_target() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut ui = window();
        ui.pause = Duration::from_millis(50);
        let (mut a, mut a_peer) = loopback().await;
        let (mut b, mut b_peer) = loopback().await;
        tokio::join!(
            login_with_hash(&ui, "pw", hash("a"), "a", 1, false, &mut a),
            login_with_hash(&ui, "pw", hash("b"), "b", 2, false, &mut b),
        );
        assert_eq!(target(&login_at(&mut a_peer).await), ("a".to_owned(), 1));
        assert_eq!(target(&login_at(&mut b_peer).await), ("b".to_owned(), 2));
    });
}

#[test]
fn a_mapping_answers_the_prompt_with_its_own_challenge() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let ui = window();
        let (mut a, mut a_peer) = loopback().await;
        let (mut b, mut b_peer) = loopback().await;
        // A's hash arrived last, so it is the one the handler holds.
        assert!(login_with_hash(&ui, "pw", hash("a"), "a", 1, false, &mut a).await);
        login_at(&mut a_peer).await;
        let typed = (String::new(), String::new(), "pw".to_owned(), false);
        login_from_ui(&ui, &hash("b"), typed, "b", 2, false, &mut b).await;
        let lr = login_at(&mut b_peer).await;
        assert_eq!(lr.password, digest("b"));
        assert_eq!(target(&lr), ("b".to_owned(), 2));
    });
}

#[test]
fn a_password_typed_before_this_connections_hash_answers_it_when_it_comes() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let ui = window();
        let (mut b, mut b_peer) = loopback().await;
        // The prompt's password reached B before its hash, and no other
        // mapping has stored it in the handler yet.
        let typed = (String::new(), String::new(), "pw".to_owned(), false);
        assert!(hash_arrived(&ui, "", hash("b"), Some(typed), "b", 2, false, &mut b).await);
        let lr = login_at(&mut b_peer).await;
        assert_eq!(lr.password, digest("b"));
        assert_eq!(target(&lr), ("b".to_owned(), 2));
    });
}

#[test]
fn a_raw_pipe_login_on_a_multiplexed_window_does_not_ask_for_the_tunnel() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let ui = window();
        // The window probes for the tunnel, but this mapping latched to
        // the raw pipe: its login must read as the raw pipe's, or an
        // upgraded peer answers with a tunnel it then never gets.
        ui.lc.write().unwrap().port_forward_mux = true;
        let (mut a, mut a_peer) = loopback().await;
        assert!(login_with_hash(&ui, "pw", hash("a"), "a", 1, false, &mut a).await);
        assert!(!login_at(&mut a_peer).await.port_forward().multiplex);
    });
}

#[test]
fn a_password_typed_at_the_prompt_keeps_a_raw_pipe_login_raw() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let ui = window();
        ui.lc.write().unwrap().port_forward_mux = true;
        let (mut b, mut b_peer) = loopback().await;
        let typed = (String::new(), String::new(), "pw".to_owned(), false);
        login_from_ui(&ui, &hash("b"), typed, "b", 2, false, &mut b).await;
        assert!(!login_at(&mut b_peer).await.port_forward().multiplex);
    });
}

#[test]
fn a_probing_login_asks_for_the_tunnel() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let ui = window();
        let (mut a, mut a_peer) = loopback().await;
        assert!(login_with_hash(&ui, "pw", hash("a"), "a", 1, true, &mut a).await);
        assert!(login_at(&mut a_peer).await.port_forward().multiplex);
    });
}
