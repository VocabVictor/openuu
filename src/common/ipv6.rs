use super::*;

#[inline]
pub fn is_udp_disabled() -> bool {
    Config::get_option(keys::OPTION_DISABLE_UDP) == "Y"
}

/// Run KCP with its congestion window (nc=0) instead of the turbo profile it has always shipped.
///
/// Opt-in: which profile wins depends on why packets are lost — nc=1 deepens real congestion,
/// while nc=0 reads random loss as congestion and its RTO backoff drops cwnd to 1. Undecidable
/// without a shaped link, so keep what users run today.
#[inline]
pub fn get_kcp_cc_enabled() -> bool {
    let k = keys::OPTION_ALLOW_KCP_CC;
    config::option2bool(k, &Config::get_option(k))
}

// this crate https://github.com/yoshd/stun-client supports nat type
async fn stun_ipv6_test(stun_server: String) -> ResultType<(SocketAddr, String)> {
    use stunclient::StunClient;
    let local_addr = SocketAddr::from(([0u16; 8], 0)); // [::]:0
    let socket = UdpSocket::bind(&local_addr).await?;
    // Resolve via tokio so DNS never blocks the async runtime worker.
    let Some(stun_addr) = tokio::net::lookup_host(&stun_server)
        .await?
        .find(|x| x.is_ipv6())
    else {
        bail!(
            "Failed to resolve STUN ipv6 server address: {}",
            stun_server
        );
    };
    let client = StunClient::new(stun_addr);
    let addr = client.query_external_address_async(&socket).await?;
    Ok(if addr.ip().is_ipv6() {
        (addr, stun_server)
    } else {
        bail!("STUN server returned non-IPv6 address: {}", addr)
    })
}

async fn test_bind_ipv6() -> ResultType<SocketAddr> {
    use hbb_common::futures::future::FutureExt;
    let local_addr = SocketAddr::from(([0u16; 8], 0)); // [::]:0
    let socket = UdpSocket::bind(local_addr).await?;
    // Nothing is sent - `connect` only makes the kernel pick a route and a source address - so any
    // resolvable target answers equally and the whole cost is DNS. Race the lookups rather than
    // walk them: this is awaited inline on the connection path, not every STUN host publishes a
    // AAAA, and one resolver that hangs must not decide whether this host has v6.
    let lookups = hbb_common::webrtc::WebRTCStream::default_stun_servers()
        .into_iter()
        .map(|stun| {
            (async move {
                let addr = tokio::net::lookup_host(&stun)
                    .await?
                    .find(|x| x.is_ipv6())
                    .ok_or_else(|| {
                        anyhow!("Failed to resolve STUN ipv6 server address: {}", stun)
                    })?;
                Ok::<SocketAddr, hbb_common::anyhow::Error>(addr)
            })
            .boxed()
        })
        .collect::<Vec<_>>();
    let (addr, _) = hbb_common::futures::future::select_ok(lookups).await?;
    socket.connect(addr).await?;
    Ok(socket.local_addr()?)
}

pub async fn test_ipv6() -> Option<tokio::task::JoinHandle<()>> {
    if PUBLIC_IPV6_ADDR
        .lock()
        .unwrap()
        .1
        .map(|x| x.elapsed().as_secs() < 60)
        .unwrap_or(false)
    {
        return None;
    }
    PUBLIC_IPV6_ADDR.lock().unwrap().1 = Some(Instant::now());

    match test_bind_ipv6().await {
        Ok(mut addr) => {
            if let std::net::IpAddr::V6(ip) = addr.ip() {
                if !ip.is_loopback()
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                    && (ip.segments()[0] & 0xe000) == 0x2000
                {
                    addr.set_port(0);
                    PUBLIC_IPV6_ADDR.lock().unwrap().0 = Some(addr);
                    log::debug!("Found public IPv6 address locally: {}", addr);
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to bind IPv6 socket: {}", e);
        }
    }
    // Interestingly, on my macOS, sometimes my ipv6 works, sometimes not (test with ping6 or https://test-ipv6.com/).
    // I checked ifconfig, could not see any difference. Both secure ipv6 and temporary ipv6 are there.
    // So we can not rely on the local ipv6 address queries with if_addrs.
    // above test_bind_ipv6 is safer, because it can fail in this case.
    /*
    std::thread::spawn(|| {
        if let Ok(ifaces) = if_addrs::get_if_addrs() {
            for iface in ifaces {
                if let if_addrs::IfAddr::V6(v6) = iface.addr {
                    let ip = v6.ip;
                    if !ip.is_loopback()
                        && !ip.is_unspecified()
                        && !ip.is_multicast()
                        && !ip.is_unique_local()
                        && !ip.is_unicast_link_local()
                        && (ip.segments()[0] & 0xe000) == 0x2000
                    {
                        // only use the first one, on mac, the first one is the stable
                        // one, the last one is the temporary one. The middle ones are deperecated.
                        *PUBLIC_IPV6_ADDR.lock().unwrap() =
                            Some((SocketAddr::from((ip, 0)), Instant::now()));
                        log::debug!("Found public IPv6 address locally: {}", ip);
                        break;
                    }
                }
            }
        }
    });
    */

    Some(tokio::spawn(async {
        use hbb_common::futures::future::{select_ok, FutureExt};
        let tests = hbb_common::webrtc::WebRTCStream::default_stun_servers()
            .into_iter()
            .map(|stun| stun_ipv6_test(stun).boxed())
            .collect::<Vec<_>>();

        match select_ok(tests).await {
            Ok(res) => {
                let mut addr = res.0 .0;
                addr.set_port(0); // Set port to 0 to avoid conflicts
                PUBLIC_IPV6_ADDR.lock().unwrap().0 = Some(addr);
                log::debug!(
                    "Found public IPv6 address via STUN server {}: {}",
                    res.0 .1,
                    addr
                );
            }
            Err(e) => {
                log::error!("Failed to get public IPv6 address: {}", e);
            }
        };
    }))
}
