use std::sync::{Arc, RwLock};

use crate::client::*;
use crate::port_forward_mux::{Claim, Tunnel, CHANNEL_WINDOW};
use hbb_common::{
    allow_err, bail,
    config::READ_TIMEOUT,
    futures::{SinkExt, StreamExt},
    log,
    protobuf::Message as _,
    rendezvous_proto::ConnType,
    tcp, timeout,
    tokio::{self, net::TcpStream, sync::mpsc},
    tokio_util::codec::{BytesCodec, Framed},
    ResultType, Stream,
};
use base::message_proto::*;

mod login;
mod mux;
pub use mux::*;
use login::{connect_and_login, hash_arrived, login_from_ui};
#[cfg(test)]
use login::login_with_hash;
use mux::{establish_tunnel, run_forward};
#[cfg(test)]
use mux::{peer_supports_mux, take_socket};

fn run_rdp(port: u16, name: &str) {
    std::process::Command::new("cmdkey")
        .arg("/delete:localhost")
        .output()
        .ok();
    let username = std::env::var("rdp_username").unwrap_or_default();
    let password = std::env::var("rdp_password").unwrap_or_default();
    if !username.is_empty() || !password.is_empty() {
        let mut args = vec!["/generic:localhost".to_owned()];
        if !username.is_empty() {
            args.push(format!("/user:{}", username));
        }
        if !password.is_empty() {
            args.push(format!("/pass:{}", password));
        }
        std::process::Command::new("cmdkey")
            .args(&args)
            .output()
            .ok();
    }
    // Keep using /v instead of a generated .rdp file: mstsc then preserves the
    // user's Default.rdp settings and avoids unsigned-file warnings or policies.
    match std::process::Command::new("mstsc")
        .arg(format!("/v:localhost:{}", port))
        .spawn()
    {
        Ok(child) => {
            #[cfg(windows)]
            crate::platform::set_rdp_window_title(child, name.to_owned());
            #[cfg(not(windows))]
            let _ = (child, name);
        }
        Err(err) => log::warn!("Failed to launch mstsc: {}", err),
    }
}

// Show the peer identity with its hostname, using the ID when no alias exists.
fn rdp_display_name(lc: &Arc<RwLock<LoginConfigHandler>>, id: &str) -> String {
    let lc = lc.read().unwrap();
    let alias = lc
        .options
        .get("alias")
        .map(|s| s.trim())
        .unwrap_or_default();
    let hostname = lc.info.hostname.trim();
    let identity = if !alias.is_empty() { alias } else { id };
    if hostname.is_empty() || hostname == identity {
        identity.to_owned()
    } else {
        format!("{} ({})", identity, hostname)
    }
}

pub async fn listen(
    id: String,
    password: String,
    port: i32,
    interface: impl Interface,
    ui_receiver: mpsc::UnboundedReceiver<Data>,
    key: &str,
    token: &str,
    lc: Arc<RwLock<LoginConfigHandler>>,
    remote_host: String,
    remote_port: i32,
) -> ResultType<()> {
    crate::account::require_login().await?;
    let listener = tcp::new_listener(format!("127.0.0.1:{}", port), true).await?;
    let addr = listener.local_addr()?;
    log::info!("listening on port {:?}", addr);
    let is_rdp = port == 0;
    if is_rdp {
        run_rdp(addr.port(), &rdp_display_name(&lc, &id));
    }
    let mut ui_receiver = ui_receiver;
    // One tunnel per mapping; the listener drops it on its way out, and that
    // ends the tunnel.
    let tunnel = Tunnel::new();
    loop {
        tokio::select! {
            Ok((forward, addr)) = listener.accept() => {
                log::info!("new connection from {:?}", addr);
                // A multiplexed window takes the connection on the mapping's
                // tunnel, or probes for one on its first accept. Everything
                // else, the setting off or a peer without the feature, is the
                // raw pipe below, as it always was.
                let claim = if lc.read().unwrap().port_forward_mux { tunnel.claim() } else { Claim::Legacy };
                match claim {
                    Claim::Muxed(handle) => {
                        if let Err(e) = handle.open(&remote_host, remote_port, forward, Vec::new()) {
                            log::debug!("cannot open channel for {:?}: {}", addr, e);
                        }
                        continue;
                    }
                    Claim::Claimed => {
                        if establish_tunnel(&tunnel, &id, &password, &mut ui_receiver, &interface, forward, addr, key, token, is_rdp, &remote_host, remote_port).await {
                            break;
                        }
                        continue;
                    }
                    Claim::Legacy => {}
                }
                let id = id.clone();
                let password = password.clone();
                let mut forward = Framed::new(forward, BytesCodec::new());
                let mut close_port_forward = false;
                match connect_and_login(&id, &password, &mut ui_receiver, interface.clone(), &mut forward, key, token, is_rdp, &mut close_port_forward, &remote_host, remote_port).await {
                    Ok(Some(stream)) => {
                        let interface = interface.clone();
                        tokio::spawn(async move {
                            if let Err(err) = run_forward(forward, stream).await {
                                interface.msgbox("error", "Error", &err.to_string(), "");
                            }
                            log::info!("connection from {:?} closed", addr);
                       });
                    }
                    _ if close_port_forward => {
                        break;
                    }
                    Err(err) => {
                        interface.on_establish_connection_error(err.to_string());
                    }
                    _ => {}
                }
            }
            d = ui_receiver.recv() => {
                match d {
                    Some(Data::Close) => {
                        break;
                    }
                    Some(Data::NewRDP) => {
                        println!("receive run_rdp from ui_receiver");
                        run_rdp(addr.port(), &rdp_display_name(&lc, &id));
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod login_tests {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_supports_mux_reads_the_features_bit() {
        let mut pi = PeerInfo::new();
        assert!(!peer_supports_mux(&pi));
        pi.features = Some(Features { port_forward_mux: false, ..Default::default() }).into();
        assert!(!peer_supports_mux(&pi));
        pi.features = Some(Features { port_forward_mux: true, ..Default::default() }).into();
        assert!(peer_supports_mux(&pi));
    }

    #[test]
    fn port_forward_mux_defaults_to_on() {
        use hbb_common::config::option2bool;
        use base::config::keys;
        // option2bool's fallback branch is also "on unless N", so the value
        // assertions below would pass for a prefixless key too. The `enable-`
        // prefix is what actually guarantees the default, and renaming the key
        // to an `allow-` one would silently flip it — pin the prefix itself.
        assert!(keys::OPTION_ENABLE_PORT_FORWARD_MUX.starts_with("enable-"));
        assert!(option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, ""));
        assert!(option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, "Y"));
        assert!(!option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, "N"));
    }

    #[test]
    fn take_socket_hands_back_a_working_socket_and_the_prebuf() {
        use hbb_common::tokio::io::{AsyncReadExt, AsyncWriteExt};
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = l.local_addr().unwrap();
            let mut client = TcpStream::connect(addr).await.unwrap();
            let (server, _) = l.accept().await.unwrap();
            let mut framed = Framed::new(server, BytesCodec::new());
            client.write_all(b"abc").await.unwrap();
            // Read through the codec, as connect_and_login does during login.
            let pulled = framed.next().await.unwrap().unwrap();
            assert_eq!(&pulled[..], b"abc");
            let (mut sock, prebuf) = take_socket(framed, pulled.to_vec());
            assert_eq!(prebuf, b"abc".to_vec());
            // Bytes written after the handoff arrive on the bare socket.
            client.write_all(b"def").await.unwrap();
            let mut buf = [0u8; 3];
            sock.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"def");
        });
    }
}
