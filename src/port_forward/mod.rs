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
#[cfg(test)]
mod login_tests;
#[cfg(test)]
mod tests;
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


