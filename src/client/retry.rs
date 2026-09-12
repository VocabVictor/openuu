use super::*;

/// Check if the given message is an error and can be retried.
///
/// # Arguments
///
/// * `msgtype` - The message type.
/// * `title` - The title of the message.
/// * `text` - The text of the message.
#[inline]
pub fn check_if_retry(msgtype: &str, title: &str, text: &str, retry_for_relay: bool) -> bool {
    msgtype == "error"
        && title == "Connection Error"
        && ((text.contains("10054") || text.contains("104")) && retry_for_relay
            || (!text.to_lowercase().contains("offline")
                && !text.to_lowercase().contains("not exist")
                && (!text.to_lowercase().contains("handshake")
                    // https://github.com/snapview/tungstenite-rs/blob/e7e060a89a72cb08e31c25a6c7284dc1bd982e23/src/error.rs#L248
                    || text
                        .to_lowercase()
                        .contains("connection reset without closing handshake") && use_ws())
                && !text.to_lowercase().contains("failed")
                && !text.to_lowercase().contains("resolve")
                && !text.to_lowercase().contains("mismatch")
                && !text.to_lowercase().contains("manually")
                && !text.to_lowercase().contains("restricted")
                && !text.to_lowercase().contains("incoming only")
                && !text.to_lowercase().contains("not allowed")))
}

#[cfg(test)]
mod retry_tests {
    use super::check_if_retry;

    #[test]
    fn incoming_only_error_is_not_retryable() {
        assert!(!check_if_retry(
            "error",
            "Connection Error",
            "Incoming only mode",
            false,
        ));
    }
}

#[cfg(test)]
mod port_forward_mux_tests {
    use super::*;

    #[test]
    fn a_login_asks_for_the_tunnel_when_its_mapping_probes() {
        let mut lc = LoginConfigHandler::default();
        lc.conn_type = ConnType::PORT_FORWARD;
        let asks = |lc: &LoginConfigHandler| {
            lc.create_login_msg(String::new(), String::new(), vec![])
                .login_request()
                .port_forward()
                .multiplex
        };
        assert!(!asks(&lc));
        lc.port_forward_multiplex = true;
        assert!(asks(&lc));
    }
}
