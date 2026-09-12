use super::*;

/// The first accept of a multiplexed mapping. It logs in asking for the
/// tunnel, and the peer's answer fixes this listener's mode until it closes:
/// a peer with the feature gets a tunnel every later accept joins, one
/// without gets today's raw pipe for this connection and `Legacy` for the
/// rest. Re-adding the mapping is how a user picks up an upgraded peer;
/// nothing switches modes underneath live connections. Returns `true` when
/// the listener should stop.
pub(super) async fn establish_tunnel(
    tunnel: &Tunnel,
    id: &str,
    password: &str,
    ui_receiver: &mut mpsc::UnboundedReceiver<Data>,
    interface: &impl Interface,
    forward: TcpStream,
    addr: std::net::SocketAddr,
    key: &str,
    token: &str,
    is_rdp: bool,
    remote_host: &str,
    remote_port: i32,
) -> bool {
    let mut forward = Framed::new(forward, BytesCodec::new());
    let mut close_port_forward = false;
    match connect_and_login_mux(id, password, ui_receiver, interface.clone(), &mut forward, key, token, is_rdp, &mut close_port_forward, remote_host, remote_port).await {
        Ok(Some(outcome)) if outcome.mux => {
            let handle = tunnel.set_muxed(outcome.stream, interface.clone());
            if !outcome.local_eof {
                let (socket, prebuf) = take_socket(forward, outcome.prebuf);
                if let Err(e) = handle.open(remote_host, remote_port, socket, prebuf) {
                    log::debug!("cannot open channel for {:?}: {}", addr, e);
                }
            }
        }
        Ok(Some(outcome)) => {
            tunnel.set_legacy();
            if outcome.local_eof {
                log::debug!("legacy peer and local {:?} already gone", addr);
            } else {
                run_legacy(outcome, forward, addr, interface.clone());
            }
        }
        _ if close_port_forward => {
            tunnel.set_failed();
            return true;
        }
        Err(err) => {
            tunnel.set_failed();
            interface.on_establish_connection_error(err.to_string());
        }
        _ => tunnel.set_failed(),
    }
    false
}

/// `connect_and_login` for a mapping that wants the tunnel: the pre-read
/// stops at one window rather than growing without bound, and a local EOF
/// no longer ends the login, since the tunnel may still be wanted. It
/// reports what the peer answered rather than a raw stream, because the
/// caller's next step depends on it. The login itself is the raw pipe's,
/// told to ask for the tunnel.
async fn connect_and_login_mux(
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
) -> ResultType<Option<LoginOutcome>> {
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
    let mut local_eof = false;
    let mux;
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
                            if !hash_arrived(&interface, password, hash, pending_login.take(), remote_host, remote_port, true, &mut stream).await {
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
                                mux = peer_supports_mux(&pi);
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
                        Some(hash) => login_from_ui(&interface, hash, login, remote_host, remote_port, true, &mut stream).await,
                        None => pending_login = Some(login),
                    },
                    Some(Data::Message(msg)) => {
                        allow_err!(stream.send(&msg).await);
                    }
                    _ => {}
                }
            },
            // Stop pulling once the pre-read buffer is a window deep; the
            // rest waits in the kernel until the channel opens. A local EOF
            // no longer aborts the login: the tunnel may still be wanted.
            res = forward.next(), if !local_eof && buffer.len() < CHANNEL_WINDOW as usize => {
                if let Some(Ok(bytes)) = res {
                    buffer.extend(bytes);
                } else {
                    local_eof = true;
                }
            },
        }
    }
    Ok(Some(LoginOutcome {
        stream,
        mux,
        prebuf: buffer,
        local_eof,
    }))
}

/// Today's raw pipe, for peers without multiplexing.
fn run_legacy(
    outcome: LoginOutcome,
    forward: Framed<TcpStream, BytesCodec>,
    addr: std::net::SocketAddr,
    interface: impl Interface,
) {
    let mut stream = outcome.stream;
    let prebuf = outcome.prebuf;
    tokio::spawn(async move {
        stream.set_raw();
        if !prebuf.is_empty() {
            allow_err!(stream.send_bytes(prebuf.into()).await);
        }
        if let Err(err) = run_forward(forward, stream).await {
            interface.msgbox("error", "Error", &err.to_string(), "");
        }
        log::info!("connection from {:?} closed", addr);
    });
}

struct LoginOutcome {
    stream: Stream,
    mux: bool,
    prebuf: Vec<u8>,
    local_eof: bool,
}

pub(super) fn peer_supports_mux(pi: &PeerInfo) -> bool {
    pi.features.as_ref().map(|f| f.port_forward_mux).unwrap_or(false)
}

/// `into_inner()` would drop bytes the codec pulled but never yielded.
pub(super) fn take_socket(forward: Framed<TcpStream, BytesCodec>, mut prebuf: Vec<u8>) -> (TcpStream, Vec<u8>) {
    let parts = forward.into_parts();
    prebuf.extend_from_slice(&parts.read_buf);
    (parts.io, prebuf)
}

/// The controlling side's `enable-port-forward-mux`: on unless set to `N`.
pub fn mux_enabled() -> bool {
    use hbb_common::config::{option2bool, LocalConfig};
    use base::config::keys;
    option2bool(
        keys::OPTION_ENABLE_PORT_FORWARD_MUX,
        &LocalConfig::get_option(keys::OPTION_ENABLE_PORT_FORWARD_MUX),
    )
}

pub(super) async fn run_forward(forward: Framed<TcpStream, BytesCodec>, stream: Stream) -> ResultType<()> {
    log::info!("new port forwarding connection started");
    let mut forward = forward;
    let mut stream = stream;
    let mut account_timer = hbb_common::tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = account_timer.tick() => { crate::account::require_login().await?; },
            res = forward.next() => {
                if let Some(Ok(bytes)) = res {
                    allow_err!(stream.send_bytes(bytes.into()).await);
                } else {
                    break;
                }
            },
            res = stream.next() => {
                if let Some(Ok(bytes)) = res {
                    allow_err!(forward.send(bytes).await);
                } else {
                    break;
                }
            },
        }
    }
    Ok(())
}
