use super::*;

/// Handle one `_drm` consumer: a private worker thread owns the `!Send` reader; this task forwards.
pub(super) async fn handle_drm_conn(stream: Connection) -> ResultType<()> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // World-connectable socket, so the peer MUST be authorized here (this listener bypasses the
    // generic `start()` accept loop). On the blocking pool: a cache miss forks `loginctl`.
    static DRM_AUTH_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
    struct DrmAuthGuard;
    impl Drop for DrmAuthGuard {
        fn drop(&mut self) {
            DRM_AUTH_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        }
    }
    if !drm_auth_admitted(DRM_AUTH_IN_FLIGHT.fetch_add(1, Ordering::SeqCst)) {
        DRM_AUTH_IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
        // Deliberately `debug`, not `warn`: this is reachable by any local uid, so a level that
        // reaches the service log on every attempt is an unbounded log-write primitive for that peer.
        log::debug!("drm: too many _drm authorizations in flight; dropping this connection");
        return Ok(());
    }
    let auth_guard = DrmAuthGuard;
    let (stream, authorized) = tokio::task::spawn_blocking(move || {
        let ok = authorize_service_scoped_ipc_connection(&stream, "_drm");
        (stream, ok)
    })
    .await?;
    drop(auth_guard);
    if !authorized {
        // Deliberately no log here: the call above already reports it -- the uid mismatch through
        // `log_rejected_service_connection`, throttled to one line per 5 s, and the executable
        // mismatch as a plain warn. A second, unthrottled warn here would be the same unbounded
        // log-write primitive.
        return Ok(());
    }

    static DRM_CONN_COUNT: AtomicUsize = AtomicUsize::new(0);
    struct DrmConnGuard;
    impl Drop for DrmConnGuard {
        fn drop(&mut self) {
            DRM_CONN_COUNT.fetch_sub(1, Ordering::SeqCst);
        }
    }
    if !drm_conn_admitted(DRM_CONN_COUNT.fetch_add(1, Ordering::SeqCst)) {
        DRM_CONN_COUNT.fetch_sub(1, Ordering::SeqCst);
        log::warn!("drm: too many concurrent _drm connections (>= {MAX_DRM_CONNS}); rejecting");
        return Ok(());
    }
    let _conn_guard = DrmConnGuard;

    // Re-authorized per frame below: DRM/KMS capture is NOT session-scoped, so unless a stream stops
    // when the active session changes the outgoing user's --server keeps receiving the incoming
    // user's screen (and the greeter in between).
    let peer_uid = stream.peer_uid();

    let mut conn = dup_to_drm_conn(&stream)?;
    drop(stream);

    let (frame_tx, mut frame_rx) = tokio::sync::mpsc::channel::<DrmProducerMsg>(2);
    let (crtc_tx, crtc_rx) = std::sync::mpsc::channel::<(String, u32, bool)>();
    let stop = Arc::new(AtomicBool::new(false));
    let _stop_guard = DrmStopGuard(stop.clone());
    let worker_stop = stop.clone();
    let frames_gated = Arc::new(AtomicBool::new(false));
    let worker_gate = frames_gated.clone();
    std::thread::Builder::new()
        .name("drm-capture".into())
        .spawn(move || drm_capture_worker(frame_tx, crtc_rx, worker_stop, worker_gate))
        .map_err(|err| anyhow::anyhow!("could not spawn the drm capture worker: {err}"))?;

    let displays = match frame_rx.recv().await {
        Some(DrmProducerMsg::Displays(d)) => d,
        _ => {
            log::info!("drm: reader unavailable; closing _drm connection (client falls back)");
            return Ok(());
        }
    };
    conn.send_msg(&Data::DrmDisplayList(displays.clone()), None).await?;

    let (display_idx, need_cpu) = match conn.recv_msg_timeout2(10_000).await {
        Some(Ok((Data::DrmStart { display, need_cpu }, _fd))) => (display, need_cpu),
        Some(Ok((_, _fd))) => {
            log::info!("drm: peer sent something other than DrmStart in the handshake; closing");
            return Ok(());
        }
        Some(Err(e)) => return Err(e),
        None => return Ok(()), // timed out: client never chose a display
    };
    // Reject crtc 0: `open(crtc=0)` auto-selects the FIRST ACTIVE CRTC and streams the WRONG monitor.
    let selected = usize::try_from(display_idx)
        .ok()
        .and_then(|i| displays.get(i));
    let target_crtc = selected.map(|d| d.crtc_id).unwrap_or(0);
    let target_device = selected.map(|d| d.device.clone()).unwrap_or_default();
    if target_crtc == 0 {
        log::warn!(
            "drm: client selected display {display_idx} with no bound CRTC; closing _drm (client falls back)"
        );
        return Ok(());
    }
    if crtc_tx.send((target_device, target_crtc, need_cpu)).is_err() {
        return Ok(());
    }

    let mut seen_gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
    pub(super) const DRM_FRAME_CREDIT: i32 = 2;
    let mut credit: i32 = DRM_FRAME_CREDIT;
    let mut credit_since = std::time::Instant::now();
    let mut held_frame: Option<DrmProducerMsg> = None;
    loop {
        conn.drain_frame_acks(&mut credit, DRM_FRAME_CREDIT)?;
        // While gated the worker does not grab, so it cannot advance its own MAX_STALLED watchdog: a
        // consumer that stops acking without closing the socket would otherwise hold this connection,
        // its worker thread and the privileged DRM context open indefinitely.
        const CREDIT_STALL: std::time::Duration = std::time::Duration::from_secs(5);
        if credit > 0 {
            credit_since = std::time::Instant::now();
        } else if credit_since.elapsed() > CREDIT_STALL {
            log::info!("drm: consumer has not acked for {CREDIT_STALL:?}; closing _drm connection");
            break;
        }
        // This must NOT also require that a frame is already held: those grabs keep the held frame
        // fresh (latest-wins below), so gating on "held" would pin whatever frame was in hand when
        // credit ran out and ship it stale once the ack lands.
        frames_gated.store(credit <= 0, Ordering::Relaxed);
        let first: Option<DrmProducerMsg> = if held_frame.is_some() && credit > 0 {
            frame_rx.try_recv().ok()
        } else if credit <= 0 {
            const CREDIT_POLL: std::time::Duration = std::time::Duration::from_secs(1);
            let waited = tokio::time::timeout(CREDIT_POLL, async {
                tokio::select! {
                    biased;
                    r = conn.wait_readable() => r.map(|_| None),
                    m = frame_rx.recv() => Ok(Some(m)),
                }
            })
            .await;
            match waited {
                Err(_) => None,
                Ok(Err(err)) => return Err(err),
                Ok(Ok(None)) => None,
                Ok(Ok(Some(None))) => break,
                Ok(Ok(Some(Some(m)))) => Some(m),
            }
        } else {
            match frame_rx.recv().await {
                Some(f) => Some(f),
                None => break,
            }
        };
        // Re-authorize per frame with the CACHE-ONLY active uid: a fresh lookup forks `loginctl` and
        // would stall every stream on this single-threaded runtime. A miss is fail-closed for a non-root peer
            // (root stays authorized; see `drm_peer_authorized`).
        let peer_ok = drm_peer_authorized(peer_uid, active_uid_cached());
        if !peer_ok {
            log::warn!("drm: _drm peer no longer matches the active session (or it is unknown); closing");
            break;
        }
        let gen = DRM_DISPLAY_GENERATION.load(Ordering::Acquire);
        if gen != seen_gen {
            seen_gen = gen;
            let fresh = DRM_DISPLAY_CACHE
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            // Send even an EMPTY list, or the consumer keeps advertising removed displays.
            conn.send_msg(&Data::DrmDisplaysChanged(fresh), None).await?;
        }
        let mut latest_frame: Option<DrmProducerMsg> = held_frame.take();
        let mut msg = first.or_else(|| frame_rx.try_recv().ok());
        while let Some(m) = msg.take() {
            match m {
                f @ (DrmProducerMsg::Frame { .. } | DrmProducerMsg::FrameCpu { .. }) => {
                    latest_frame = Some(f);
                }
                DrmProducerMsg::Cursor {
                    id,
                    width,
                    height,
                    hotx,
                    hoty,
                    colors,
                } => {
                    conn.send_msg(
                        &Data::DrmCursor {
                            id,
                            width,
                            height,
                            hotx,
                            hoty,
                        },
                        None,
                    )
                    .await?;
                    conn.send_raw(Bytes::from(colors)).await?;
                }
                DrmProducerMsg::Displays(_) => {}
            }
            msg = frame_rx.try_recv().ok();
        }
        conn.drain_frame_acks(&mut credit, DRM_FRAME_CREDIT)?;
        if credit <= 0 {
            held_frame = latest_frame;
            continue;
        }
        match latest_frame {
            Some(DrmProducerMsg::Frame { mut desc, fd }) => {
                // Every exported frame carries its fd: the kernel can recycle an fb_id onto another
                // buffer with the same geometry/modifier and this side cannot see the dma-buf inode
                // that would tell the difference, so eliding it can serve a stale EGLImage. libdrmtap's
                // import cache keys on fb_id AND inode, and can only re-import when handed a real fd.
                let send_fd = fd.is_some();
                desc.has_fd = send_fd;
                let borrowed = if send_fd { fd.as_ref().map(|f| f.as_fd()) } else { None };
                conn.send_msg(&Data::DrmFrameDmabuf(desc), borrowed).await?;
                credit -= 1; // one frame in flight until the consumer acks it
                // `fd` (OwnedFd) is closed here whether or not it was attached (the cmsg dup'd it
                // into the peer), which bounds our fd usage to ~1 in flight per frame.
            }
            Some(DrmProducerMsg::FrameCpu {
                width,
                height,
                data,
            }) => {
                conn.send_msg(&Data::DrmFrame { width, height }, None).await?;
                conn.send_raw(data).await?;
                credit -= 1; // one frame in flight until the consumer acks it
            }
            _ => {}
        }
    }
    Ok(())
}
