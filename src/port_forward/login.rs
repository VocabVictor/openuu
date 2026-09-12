use super::*;

pub(super) async fn connect_and_login(
    id: &str,
    password: &str,
    ui_receiver: &mut mpsc::UnboundedReceiver<Data>,
    interface: impl Interface,
    forward: &mut Framed<TcpStream, BytesCodec>,
    key: &str,
    token: &str,
    is_rdp: bool,
    close_port_forward: &mut bool,
    remote_host: &str,
    remote_port: i32,
) -> ResultType<Option<Stream>> {
    let conn_type = if is_rdp {
        ConnType::RDP
    } else {
        ConnType::PORT_FORWARD
    };
    let ((mut stream, direct, _pk, _kcp, _stream_type), (feedback, rendezvous_server)) =
        Client::start(id, key, token, conn_type, interface.clone()).await?;
    interface.update_direct(Some(direct));
    if !stream.is_secured() && !crate::common::is_direct_ip_access(id) {
        if !confirm_insecure_connection(&interface, ui_receiver).await {
            *close_port_forward = true;
            return Ok(None);
        }
    }
    let mut buffer = Vec::new();
    let mut received = false;
    let mut challenge = None;
    let mut pending_login = None;

    let _keep_it = hc_connection(feedback, rendezvous_server, token).await;

    loop {
        tokio::select! {
            res = timeout(READ_TIMEOUT, stream.next()) => match res {
                Err(_) => {
                    bail!("Timeout");
                }
                Ok(Some(Ok(bytes))) => {
                    if !received {
                        received = true;
                        interface.update_received(true);
                    }
                    let msg_in = Message::parse_from_bytes(&bytes)?;
                    match msg_in.union {
                        Some(message::Union::Hash(hash)) => {
                            challenge = Some(hash.clone());
                            if !hash_arrived(&interface, password, hash, pending_login.take(), remote_host, remote_port, false, &mut stream).await {
                                return Ok(None);
                            }
                        }
                        Some(message::Union::LoginResponse(lr)) => match lr.union {
                            Some(login_response::Union::Error(err)) => {
                                if !interface.handle_login_error(&err) {
                                    return Ok(None);
                                }
                            }
                            Some(login_response::Union::PeerInfo(pi)) => {
                                interface.handle_peer_info(pi);
                                break;
                            }
                            _ => {}
                        }
                        Some(message::Union::TestDelay(t)) => {
                            interface.handle_test_delay(t, &mut stream).await;
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(err))) => {
                    bail!("Connection closed: {}", err);
                }
                _ => {
                    bail!("Reset by the peer");
                }
            },
            d = ui_receiver.recv() => {
                match d {
                    Some(Data::Login(login)) => match &challenge {
                        Some(hash) => login_from_ui(&interface, hash, login, remote_host, remote_port, false, &mut stream).await,
                        None => pending_login = Some(login),
                    },
                    Some(Data::Message(msg)) => {
                        allow_err!(stream.send(&msg).await);
                    }
                    _ => {}
                }
            },
            res = forward.next() => {
                if let Some(Ok(bytes)) = res {
                    buffer.extend(bytes);
                } else {
                    return Ok(None);
                }
            },
        }
    }
    stream.set_raw();
    if !buffer.is_empty() {
        allow_err!(stream.send_bytes(buffer.into()).await);
    }
    Ok(Some(stream))
}


/// A mapping's login is built from the window's shared handler:
/// `create_login_msg` reads `port_forward` and `port_forward_multiplex`,
/// `handle_login_from_ui` reads `hash`. Mappings log in concurrently, so each
/// fills them and sends under the window's turn lock, or one login carried
/// another mapping's target or answered another's challenge.
pub(super) async fn login_with_hash(
    interface: &impl Interface,
    password: &str,
    hash: Hash,
    remote_host: &str,
    remote_port: i32,
    mux: bool,
    stream: &mut Stream,
) -> bool {
    let lc = interface.get_lch();
    let turn = lc.read().unwrap().port_forward_login_turn.clone();
    let _turn = turn.lock().await;
    lc.write().unwrap().port_forward = (remote_host.to_owned(), remote_port);
    lc.write().unwrap().port_forward_multiplex = mux;
    interface.handle_hash(password, hash, stream).await
}

type UiLogin = (String, String, String, bool);

/// This connection's `Hash`. The window's password prompt is broadcast to
/// every mapping and can reach this one first, so a password typed while
/// the `Hash` was on its way is kept and answers it now, rather than being
/// dropped in the hope that the mapping which prompted has already stored
/// it in the shared handler.
pub(super) async fn hash_arrived(
    interface: &impl Interface,
    password: &str,
    hash: Hash,
    pending_login: Option<UiLogin>,
    remote_host: &str,
    remote_port: i32,
    mux: bool,
    stream: &mut Stream,
) -> bool {
    match pending_login {
        Some(login) => {
            login_from_ui(interface, &hash, login, remote_host, remote_port, mux, stream).await;
            true
        }
        None => login_with_hash(interface, password, hash, remote_host, remote_port, mux, stream).await,
    }
}

/// The window's password prompt is broadcast to every mapping; this one
/// answers it with its own challenge.
pub(super) async fn login_from_ui(
    interface: &impl Interface,
    hash: &Hash,
    login: UiLogin,
    remote_host: &str,
    remote_port: i32,
    mux: bool,
    stream: &mut Stream,
) {
    let lc = interface.get_lch();
    let turn = lc.read().unwrap().port_forward_login_turn.clone();
    let _turn = turn.lock().await;
    {
        let mut lc = lc.write().unwrap();
        lc.port_forward = (remote_host.to_owned(), remote_port);
        lc.port_forward_multiplex = mux;
        lc.set_hash(hash.clone());
    }
    let (os_username, os_password, password, remember) = login;
    interface
        .handle_login_from_ui(os_username, os_password, password, remember, stream)
        .await;
}
