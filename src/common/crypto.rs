use super::*;

pub async fn get_key(sync: bool) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.key.is_empty() {
            return lic.key;
        }
    }
    #[cfg(target_os = "ios")]
    let mut key = Config::get_option("key");
    #[cfg(not(target_os = "ios"))]
    let mut key = if sync {
        Config::get_option("key")
    } else {
        let mut options = crate::ipc::get_options_async().await;
        options.remove("key").unwrap_or_default()
    };
    if key.is_empty() {
        key = config::RS_PUB_KEY.to_owned();
    }
    key
}

pub fn pk_to_fingerprint(pk: Vec<u8>) -> String {
    let s: String = pk.iter().map(|u| format!("{:02x}", u)).collect();
    s.chars()
        .enumerate()
        .map(|(i, c)| {
            if i > 0 && i % 4 == 0 {
                format!(" {}", c)
            } else {
                format!("{}", c)
            }
        })
        .collect()
}

#[inline]
pub async fn get_next_nonkeyexchange_msg(
    conn: &mut Stream,
    timeout: Option<u64>,
) -> Option<RendezvousMessage> {
    let timeout = timeout.unwrap_or(READ_TIMEOUT);
    for _ in 0..2 {
        if let Some(Ok(bytes)) = conn.next_timeout(timeout).await {
            if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                match &msg_in.union {
                    Some(rendezvous_message::Union::KeyExchange(_)) => {
                        continue;
                    }
                    _ => {
                        return Some(msg_in);
                    }
                }
            }
        }
        break;
    }
    None
}

async fn secure_tcp_impl(conn: &mut Stream, key: &str, log_on_success: bool) -> ResultType<()> {
    // Skip additional encryption when using WebSocket connections (wss://)
    // as WebSocket Secure (wss://) already provides transport layer encryption.
    // This doesn't affect the end-to-end encryption between clients,
    // it only avoids redundant encryption between client and server.
    if use_ws() {
        return Ok(());
    }
    let rs_pk = get_rs_pk(key);
    let Some(rs_pk) = rs_pk else {
        bail!("Handshake failed: invalid public key from rendezvous server");
    };
    match timeout(READ_TIMEOUT, conn.next()).await? {
        Some(Ok(bytes)) => {
            if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                match msg_in.union {
                    Some(rendezvous_message::Union::KeyExchange(ex)) => {
                        if ex.keys.len() != 1 {
                            bail!("Handshake failed: invalid key exchange message");
                        }
                        let their_pk_b = sign::verify(&ex.keys[0], &rs_pk)
                            .map_err(|_| anyhow!("Signature mismatch in key exchange"))?;
                        let (asymmetric_value, symmetric_value, key) = create_symmetric_key_msg(
                            get_pk(&their_pk_b)
                                .context("Wrong their public length in key exchange")?,
                        );
                        let mut msg_out = RendezvousMessage::new();
                        msg_out.set_key_exchange(KeyExchange {
                            keys: vec![asymmetric_value, symmetric_value],
                            ..Default::default()
                        });
                        timeout(CONNECT_TIMEOUT, conn.send(&msg_out)).await??;
                        conn.set_key(key);
                        if log_on_success {
                            log::info!("Connection secured");
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub async fn secure_tcp(conn: &mut Stream, key: &str) -> ResultType<()> {
    secure_tcp_impl(conn, key, true).await
}

pub(super) async fn secure_tcp_silent(conn: &mut Stream, key: &str) -> ResultType<()> {
    secure_tcp_impl(conn, key, false).await
}

#[inline]
fn get_pk(pk: &[u8]) -> Option<[u8; 32]> {
    if pk.len() == 32 {
        let mut tmp = [0u8; 32];
        tmp[..].copy_from_slice(&pk);
        Some(tmp)
    } else {
        None
    }
}

#[inline]
pub fn get_rs_pk(str_base64: &str) -> Option<sign::PublicKey> {
    if let Ok(pk) = crate::decode64(str_base64) {
        get_pk(&pk).map(|x| sign::PublicKey(x))
    } else {
        None
    }
}

pub fn decode_id_pk(signed: &[u8], key: &sign::PublicKey) -> ResultType<(String, [u8; 32])> {
    let (id, pk, _) = decode_id_pk_dtls(signed, key)?;
    Ok((id, pk))
}

/// Like [`decode_id_pk`] but also returns the signed DTLS certificate fingerprint (empty string
/// for non-WebRTC peers), used to bind a WebRTC DTLS channel to the verified peer identity.
pub fn decode_id_pk_dtls(
    signed: &[u8],
    key: &sign::PublicKey,
) -> ResultType<(String, [u8; 32], String)> {
    let res = IdPk::parse_from_bytes(
        &sign::verify(signed, key).map_err(|_| anyhow!("Signature mismatch"))?,
    )?;
    if let Some(pk) = get_pk(&res.pk) {
        Ok((res.id, pk, res.dtls_fingerprint))
    } else {
        bail!("Wrong their public length");
    }
}

pub fn create_symmetric_key_msg(their_pk_b: [u8; 32]) -> (Bytes, Bytes, secretbox::Key) {
    let their_pk_b = box_::PublicKey(their_pk_b);
    let (our_pk_b, out_sk_b) = box_::gen_keypair();
    let key = secretbox::gen_key();
    let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);
    let sealed_key = box_::seal(&key.0, &nonce, &their_pk_b, &out_sk_b);
    (Vec::from(our_pk_b.0).into(), sealed_key.into(), key)
}
