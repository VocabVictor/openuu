use super::*;

/// Where the repeats start, and the factor they slow by. The controller's SYN arrives once, at an
/// instant we are never told, inside a window we are not told either: `Client::connect` sizes its
/// dial only after our PunchHoleSent, from its own rendezvous time and the direct failures it has
/// recorded for us - `CONNECT_TIMEOUT` between two known-asymmetric NATs that never failed, as
/// little as a second once one has. So the repeats cover our own ceiling instead, `CONNECT_TIMEOUT`,
/// which is as long as the accept below has always been willing to take a connection, and back
/// off across it: dense at the start, where every window begins and the short ones end, sparse
/// afterwards, which is `punch_udp`'s shape for the same reason.
pub(super) const PUNCH_INTERVAL: f32 = 0.15;
const PUNCH_BACKOFF: f32 = 1.5;
pub(super) const PUNCH_MAX_INTERVAL: f32 = 2.0;
/// How long a punch in flight may run past the deadline, and the only timer it runs on. A punch
/// is cancel-safe while it is still in SYN_SENT and not once the controller's SYN has crossed it:
/// the socket is then half way through a handshake, and dropping it there cuts the connection the
/// controller is opening - which its `connect` has already returned, so that attempt fails
/// outright rather than falling back to relay. A timer cannot tell the two states apart, so no
/// punch is cut on a schedule of its own, and none needs to be. A gateway that answers with RST
/// fails the connect at once, and the loop punches again. One that drops the SYN in silence
/// leaves the socket in SYN_SENT, where it holds the mapping open and the kernel re-sends the
/// SYN, and any SYN of the controller's that arrives crosses it - a second punch has nothing to
/// add. That leaves the deadline, and this much past it lets a crossing begun just before it
/// complete; Windows gives a SYN up at about 21s anyway.
pub(super) const PUNCH_GRACE: u64 = 3000;

/// The punch above leaves before hbbs has told the controller where to dial, so it is never in
/// flight at the same time as the controller's SYN: it opens our NAT, meets nothing, and a gateway
/// that answers it with RST takes the mapping down with it - leaving the listener below waiting on
/// a hole that no longer exists. Punching again across the window in which the controller dials
/// rebuilds it, and once the controller sits in SYN_SENT one of those punches meets its SYN and
/// completes as a simultaneous open: a second way in, which a single punch never had.
pub(super) async fn punch_tcp_until_connected(
    server: ServerPtr,
    peer_addr: SocketAddr,
    local_addr: SocketAddr,
    meta: ConnectionMeta,
) {
    use hbb_common::tcp::new_listener;
    // Shadows the module's `std::time::Instant`: the deadline is held against tokio's sleeps and
    // timeouts, so it runs on their clock.
    use hbb_common::tokio::time::Instant;

    // Not fatal on its own - the punch below can still meet the controller's SYN without it, and
    // that half is the one a listener the OS refused to bind could not have covered anyway.
    let listener = match new_listener(local_addr, true).await {
        Ok(listener) => {
            log::info!("Server listening on: {local_addr}");
            Some(listener)
        }
        Err(err) => {
            log::warn!("Failed to listen on {local_addr} after punching: {err}");
            None
        }
    };
    // Bounds both halves: the punch keeps the mapping open only while the accept is still
    // willing to take a connection through it.
    let until = Instant::now() + Duration::from_millis(CONNECT_TIMEOUT);
    let punch = punch_until(until, peer_addr, |ms| {
        socket_client::connect_tcp_local(peer_addr, Some(local_addr), ms)
    });
    let Some(listener) = listener else {
        if let Some(stream) = punch.await {
            serve_punched(server, stream, peer_addr, meta).await;
        }
        return;
    };
    // Accepting in a loop, not once: a transient `accept` error must not spend the whole window
    // the controller still has to arrive in.
    let accept = async {
        loop {
            let left = until.saturating_duration_since(Instant::now()).as_millis() as u64;
            if left == 0 {
                break;
            }
            match hbb_common::timeout(left, listener.accept()).await {
                // Not filtered by address, as `accept_connection` never did: hbbs saw the
                // controller through one mapping and a NAT that pools its external addresses may
                // dial us from another, and what keeps `meta`'s control permissions from a second
                // peer is the handshake, plus that exactly one connection is ever served.
                Ok(Ok(accepted)) => return Some(accepted),
                Ok(Err(err)) => {
                    log::warn!("Failed to accept from {peer_addr}: {err}");
                    // One that persists - EMFILE, say - would otherwise spin here for the window.
                    sleep(1.).await;
                }
                Err(_) => break,
            }
        }
        log::info!("Nothing connected to the hole punched to {peer_addr}");
        None
    };
    // Only the accept races the punch. Racing `accept_connection` instead would race the whole
    // session it goes on to run, so a punch landing mid-session would tear that session down.
    //
    // Whichever arrives first is the one connection this request produces. Serving the loser too
    // would give a second peer the control permissions hbbs granted for this one controller, and
    // no test on the connection itself can tell the two apart before `create_tcp_connection` has
    // spoken to it - so the invariant is kept here, by there being no second serve.
    let punched = select! {
        // Both ready at once is two connections, not one seen twice - a crossing carries the
        // punch's four-tuple, which the listener never matches - and the punch is the one kept:
        // it is known to have met something at the address hbbs gave, where the accept takes
        // any address, and dropping it would reset the connection the controller is opening.
        biased;
        Some(stream) = punch => stream,
        Some((stream, addr)) = accept => {
            return accept_punched_connection(server, stream, addr, meta).await;
        }
        else => return,
    };
    serve_punched(server, punched, peer_addr, meta).await;
}

/// The repeats of `punch_tcp_until_connected`, over any punch rather than `connect_tcp_local`
/// alone, so that a test can run the schedule against a paused clock - which no socket can be.
pub(super) async fn punch_until<T, F, Fut>(
    until: tokio::time::Instant,
    peer_addr: SocketAddr,
    mut punch: F,
) -> Option<T>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = ResultType<T>>,
{
    use hbb_common::tokio::time::Instant;

    let mut interval = PUNCH_INTERVAL;
    let mut round = 0;
    loop {
        // The deadline decides whether another punch starts, never how long one already in
        // flight may take: that one runs to PUNCH_GRACE past it.
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            log::debug!("None of {round} punches to {peer_addr} was met");
            return None;
        }
        // Cut at the deadline rather than slept out past it, so the window ends on a punch and
        // not on a gap of up to PUNCH_MAX_INTERVAL: the controller's window opened after ours,
        // on the PunchHoleSent hbbs relayed, so one as long as ours is still open through our tail.
        tokio::time::sleep(Duration::from_secs_f32(interval).min(left)).await;
        interval = (interval * PUNCH_BACKOFF).min(PUNCH_MAX_INTERVAL);
        let ms = until.saturating_duration_since(Instant::now()).as_millis() as u64 + PUNCH_GRACE;
        match punch(ms).await {
            // The controller's SYN crossed this punch, so the stream is the connection it
            // dialed, not a spare one: dropping it would reset that connection.
            Ok(stream) => return Some(stream),
            // Not logged one by one, but the count says which gateway it was: RST fails a
            // punch at once and fits a dozen into the window, a silent drop holds the one
            // punch for the whole of it. `connect_tcp_local` keeps no errno anyway.
            Err(_) => round += 1,
        }
    }
}

async fn serve_punched(
    server: ServerPtr,
    stream: Stream,
    peer_addr: SocketAddr,
    meta: ConnectionMeta,
) {
    log::info!("Punched tcp hole to {peer_addr}, connected on the punch itself");
    if let Err(err) =
        crate::server::create_tcp_connection(server, stream, peer_addr, true, meta).await
    {
        log::warn!("Failed to serve the connection punched to {peer_addr}: {err}");
    }
}

/// The accept half of `accept_connection`, kept here because only the accept may race the punch.
async fn accept_punched_connection(
    server: ServerPtr,
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    meta: ConnectionMeta,
) {
    use crate::server::create_tcp_connection;

    stream.set_nodelay(true).ok();
    match stream.local_addr() {
        Ok(stream_addr) => {
            let stream = Stream::from(stream, stream_addr);
            if let Err(err) = create_tcp_connection(server, stream, addr, true, meta).await {
                log::warn!("Failed to serve the connection from {addr}: {err}");
            }
        }
        Err(err) => log::warn!("Failed to read the address accepted from {addr}: {err}"),
    }
}
