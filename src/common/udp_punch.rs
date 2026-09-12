use super::*;

// A punch packet carries a magic and a transaction id so a reply can be *proven* to answer this
// probe. The punch it replaces sent a zero-length datagram and called the hole open on whatever
// arrived next - which the rendezvous NAT test's own leftover replies satisfied instantly, so the
// retry loop below never actually ran and its success meant nothing.
const PUNCH_PROBE: [u8; 4] = *b"RDP?";
const PUNCH_ACK: [u8; 4] = *b"RDP!";
const PUNCH_PACKET_LEN: usize = 12;

fn punch_packet(tag: &[u8; 4], tid: u64) -> [u8; PUNCH_PACKET_LEN] {
    let mut packet = [0u8; PUNCH_PACKET_LEN];
    packet[..4].copy_from_slice(tag);
    packet[4..].copy_from_slice(&tid.to_le_bytes());
    packet
}

fn punch_tid(packet: &[u8], tag: &[u8; 4]) -> Option<u64> {
    if packet.len() != PUNCH_PACKET_LEN || packet[..4] != tag[..] {
        return None;
    }
    packet[4..].try_into().ok().map(u64::from_le_bytes)
}

/// Punch until one of our own probes is acknowledged. Both ends run this identically - each
/// probes, each answers the other's probes - and each returns only once a reply carrying its own
/// transaction id comes back, the one thing that proves the pair carries traffic both ways.
///
/// Returning is therefore a fact rather than a guess, which is what lets the caller stop instead
/// of handing a dead socket to a transport whose only way to discover the truth is to time out.
///
/// A datagram that is neither probe nor acknowledgement is returned rather than dropped: it means
/// the peer finished first and is already speaking KCP, whose SYN is never retransmitted.
///
/// Only the connector stops on its own acknowledgement, because only it has something to send
/// next. An acknowledgement proves our probe came back, not that the peer's probe was answered -
/// and after this returns nothing answers probes any more, since KCP's io loop drops anything
/// shorter than its header. A listener that stopped here would go mute while a peer whose own
/// probe or answer was lost - the normal state of a hole that is still opening - kept probing an
/// endpoint that works, until it timed out. So the listener stops on the peer's first real packet.
pub async fn punch_udp(
    socket: Arc<UdpSocket>,
    listen: bool,
) -> ResultType<Option<bytes::BytesMut>> {
    let tid = ((hbb_common::time_based_rand() as u64) << 32) | hbb_common::time_based_rand() as u64;
    let probe = punch_packet(&PUNCH_PROBE, tid);
    let mut data = [0u8; 1500];
    // `connect` does not flush the receive queue, so the NAT test's extra replies are still in it.
    while socket.try_recv(&mut data).is_ok() {}

    let mut retry_interval = Duration::from_millis(20);
    const MAX_INTERVAL: Duration = Duration::from_millis(200);
    // Both ends start within one rendezvous round trip of each other and the acknowledgement is
    // one peer round trip, so a pair that has not answered in this long is not going to. The old
    // 20s came from having no way to tell "not yet" from "never".
    const MAX_TIME: Duration = Duration::from_secs(3);
    let mut probes_sent = 0u32;
    let mut probes_seen = 0u32;
    let mut acked = false;
    let mut recv_errors = 0u32;
    socket.send(&probe).await.ok();
    probes_sent += 1;
    let tm = Instant::now();
    // Absolute instants, not relative sleeps: `select!` rebuilds every arm each iteration, so a
    // peer that keeps the receive side ready restarts a relative timer before it can fire. That
    // both defeats MAX_TIME and starves the retransmit, and the peer decides the rate - an
    // old-build peer's empty datagrams match no arm below and loop without even a pause.
    let deadline = tm + MAX_TIME;
    let mut next_probe = tm + retry_interval;

    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => {
                bail!("UDP punch is timed out, {probes_sent} probes sent, {probes_seen} probes received, acked: {acked}, {recv_errors} recv errors absorbed");
            }
            _ = tokio::time::sleep_until(next_probe) => {
                socket.send(&probe).await.ok();
                probes_sent += 1;
                retry_interval = std::cmp::min(retry_interval.mul_f64(1.5), MAX_INTERVAL);
                next_probe = Instant::now() + retry_interval;
            }
            res = socket.recv(&mut data) => match res {
                Err(e) => {
                    // ICMP unreachable from the peer's NAT is expected while the hole forms and
                    // surfaces here as ConnectionReset/Refused; treat it as loss, MAX_TIME bounds
                    // the attempt. Log only the first - this retries every 10ms.
                    recv_errors += 1;
                    if recv_errors == 1 {
                        log::debug!("UDP punch recv error (treated as loss): {e}");
                    }
                    hbb_common::sleep(0.01).await;
                }
                Ok(n) => {
                    let ack = punch_tid(&data[..n], &PUNCH_ACK);
                    if ack == Some(tid) {
                        if !listen {
                            log::debug!(
                                "UDP punch confirmed in {:?}, {probes_sent} probes sent, {probes_seen} received",
                                tm.elapsed()
                            );
                            return Ok(None);
                        }
                        acked = true;
                    } else if let Some(peer_tid) = punch_tid(&data[..n], &PUNCH_PROBE) {
                        probes_seen += 1;
                        socket.send(&punch_packet(&PUNCH_ACK, peer_tid)).await.ok();
                    } else if ack.is_none() && n > 0 {
                        log::debug!(
                            "UDP punch confirmed by {n} bytes of peer data in {:?}, {probes_sent} probes sent",
                            tm.elapsed()
                        );
                        return Ok(Some(bytes::BytesMut::from(&data[..n])));
                    }
                }
            }
        }
    }
}

pub(super) fn test_ipv6_sync() {
    #[tokio::main(flavor = "current_thread")]
    async fn func() {
        if let Some(job) = test_ipv6().await {
            job.await.ok();
        }
    }
    std::thread::spawn(func);
}

pub async fn get_ipv6_socket() -> Option<(Arc<UdpSocket>, bytes::Bytes)> {
    let Some(addr) = PUBLIC_IPV6_ADDR.lock().unwrap().0 else {
        return None;
    };

    match UdpSocket::bind(addr).await {
        Err(err) => {
            log::warn!("Failed to create UDP socket for IPv6: {err}");
        }
        Ok(socket) => {
            if let Ok(local_addr_v6) = socket.local_addr() {
                return Some((
                    Arc::new(socket),
                    hbb_common::AddrMangle::encode(local_addr_v6).into(),
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
