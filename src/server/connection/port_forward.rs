use super::*;

impl Connection {
    pub(in crate::server) fn normalize_port_forward_target(pf: &mut PortForward) -> (String, bool) {
        let mut is_rdp = false;
        if pf.host == "RDP" && pf.port == 0 {
            pf.host = "localhost".to_owned();
            pf.port = 3389;
            is_rdp = true;
        }
        if pf.host.is_empty() {
            pf.host = "localhost".to_owned();
        }
        (format!("{}:{}", pf.host, pf.port), is_rdp)
    }

    pub(super) async fn connect_port_forward_if_needed(&mut self) -> bool {
        if self.is_port_forward() {
            return true;
        }
        let Some(login_request::Union::PortForward(pf)) = self.lr.union.as_ref() else {
            return true;
        };
        if pf.multiplex {
            // Port forwarding never splits, so the socket is always whole here.
            if let Some(s) = self.stream.whole() {
                crate::port_forward_mux::cap_packet_size(s);
            }
            // `inner.tx` is set for the connection's whole life; `None` here is
            // unreachable, and refusing the login is the only honest answer.
            self.port_forward_mux = self.inner.tx.clone().map(|tx| {
                super::super::port_forward_mux::PortForwardMux::new(tx, self.port_forward_address.clone())
            });
            return self.port_forward_mux.is_some();
        }
        let mut pf = pf.clone();
        let (mut addr, is_rdp) = Self::normalize_port_forward_target(&mut pf);
        self.port_forward_address = addr.clone();
        match timeout(3000, TcpStream::connect(&addr)).await {
            Ok(Ok(sock)) => {
                self.port_forward_socket = Some(Framed::new(sock, BytesCodec::new()));
                true
            }
            Ok(Err(e)) => {
                log::warn!("Port forward connect failed for {}: {}", addr, e);
                if is_rdp {
                    addr = "RDP".to_owned();
                }
                self.send_login_error(format!(
                    "Failed to access remote {}. Please make sure it is reachable/open.",
                    addr
                ))
                .await;
                false
            }
            Err(e) => {
                log::warn!("Port forward connect timed out for {}: {}", addr, e);
                if is_rdp {
                    addr = "RDP".to_owned();
                }
                self.send_login_error(format!(
                    "Failed to access remote {}. Please make sure it is reachable/open.",
                    addr
                ))
                .await;
                false
            }
        }
    }
}
