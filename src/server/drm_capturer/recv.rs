use super::*;

#[tokio::main(flavor = "current_thread")]
pub(super) async fn recv_thread(
    display: i32,
    expected: Option<DrmDisplayInfo>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<ResultType<(Vec<DrmDisplayInfo>, usize)>>,
) {
    let cursor_epoch = next_cursor_epoch();
    let mut conn = match connect_drm(DRM_CONNECT_TIMEOUT_MS).await {
        Ok(c) => c,
        Err(err) => {
            let _ = tx.send(Err(err));
            return;
        }
    };
    let displays = match conn.recv_msg_timeout2(DISPLAY_LIST_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => v,
        Some(Ok((other, _fd))) => {
            let _ = tx.send(Err(anyhow!("expected DrmDisplayList, got {:?}", other)));
            return;
        }
        Some(Err(err)) => {
            let _ = tx.send(Err(err));
            return;
        }
        None => {
            let _ = tx.send(Err(anyhow!("timed out waiting for DrmDisplayList")));
            return;
        }
    };
    // Our monitor's index IN THIS CONNECTION'S LIST; `display` indexes the CLIENT's. Measured on a
    // T2: a woken 2880x1800 panel re-enters ahead of the Touch Bar, flipping index 0.
    let wire_idx = match &expected {
        Some(e) => {
            match displays
                .iter()
                .position(|d| d.device == e.device && d.name == e.name)
            {
                Some(i) => i,
                None => {
                    let _ = tx.send(Err(anyhow!(
                        "display {display} ({}) is no longer in the service's list; \
                         the video service will rebuild against the fresh topology",
                        e.name
                    )));
                    return;
                }
            }
        }
        None => {
            let _ = tx.send(Err(anyhow!(
                "display {display} is not in the advertised list; not guessing a monitor for it"
            )));
            return;
        }
    };
    // (device, crtc_id) survives a topology change; list indices do not.
    let bound_to = displays
        .get(wire_idx)
        .map(|d| (d.device.clone(), d.crtc_id));
    let our_key = displays.get(wire_idx).map(connector_key);
    let render_node = displays
        .get(wire_idx)
        .or_else(|| displays.first())
        .map(|d| d.render_node.clone())
        .unwrap_or_default();
    // An unnamed exporter on a multi-render-node host fails SILENTLY: on a Jetson
    // (scanout nvidia-drm, first render node tegra) the wrong device's import SUCCEEDS and corrupts
    // the pixels, so there is no convert error for prefer_cpu to learn from.
    let ambiguous_gpu = render_node.is_empty() && render_node_count() > 1;
    let force_cpu = drm_prefer_cpu(&our_key) || ambiguous_gpu;
    let mut converter = if force_cpu {
        None
    } else {
        RenderConverter::open_render(Some(render_node.as_str()))
    };
    let need_cpu = converter.is_none();
    if need_cpu {
        log::info!(
            "drm: requesting the CPU-converted frame path for display {display} ({})",
            if ambiguous_gpu {
                "the service did not name the exporting GPU and this host has several render nodes; \
                 auto-selecting one can import the scanout on the wrong device and silently corrupt it"
            } else if force_cpu {
                "a prior consumer convert failed, e.g. multi-GPU render-node mismatch"
            } else {
                "no render-node convert context: libdrmtap did not load here, or \
                 drmtap_open_render found no usable /dev/dri/renderD*"
            }
        );
    }
    if let Err(err) = conn
        .send_msg(
            &Data::DrmStart {
                display: wire_idx as i32,
                need_cpu,
            },
            None,
        )
        .await
    {
        let _ = tx.send(Err(err));
        return;
    }
    let _ = tx.send(Ok((displays, wire_idx)));

    // A cursor that arrived before new() stored the session transform, held for replay. Only the
    // newest matters; the 200 ms recv timeout guarantees this is retried even on an idle wire.
    let mut pending_cursor: Option<(u64, u32, u32, i32, i32, Vec<u8>)> = None;
    let end_reason = loop {
        if stop.load(Ordering::SeqCst) {
            break "stopped".to_owned();
        }
        if pending_cursor.is_some() {
            let t = shared.transform.load(std::sync::atomic::Ordering::Acquire);
            if t != TRANSFORM_PENDING {
                if let Some((id, width, height, hotx, hoty, raw)) = pending_cursor.take() {
                    deliver_drm_cursor(display, cursor_epoch, id, width, height, hotx, hoty, raw, t);
                }
            }
        }
        let (msg, recv_fd) = match conn.recv_msg_timeout2(200).await {
            None => continue, // timeout: re-check stop at the loop top
            Some(Ok(pair)) => pair,
            Some(Err(err)) => break format!("recv: {err}"),
        };
        match msg {
            Data::DrmFrameDmabuf(desc) => {
                let conv = match converter.as_mut() {
                    Some(c) => c,
                    None => break "no DRM render node; cannot convert dma-buf frame".to_owned(),
                };
                // Valid in THIS process; -1 is an import-once cache hit on `fb_id`.
                let received_fd: RawFd = if desc.has_fd {
                    match recv_fd.as_ref() {
                        Some(f) => f.as_raw_fd(),
                        None => {
                            break "dma-buf frame set has_fd but carried no SCM_RIGHTS fd".to_owned()
                        }
                    }
                } else {
                    -1
                };
                let mut ddesc = drmtap_dmabuf_desc {
                    dma_buf_fd: -1,
                    width: desc.width,
                    height: desc.height,
                    format: desc.format,
                    modifier: desc.modifier,
                    fb_id: desc.fb_id,
                    // RAW: `drm_render::convert` REJECTS an out-of-range count rather than
                    // clamping, so the count the C reads is the one that was validated.
                    num_planes: desc.num_planes,
                    offsets: desc.offsets,
                    pitches: desc.pitches,
                    hdr_eotf: desc.hdr_eotf,
                    hdr_max_nits: desc.hdr_max_nits,
                };
                match conv.convert(&mut ddesc, received_fd) {
                    Ok((data, w, h, fmt)) => {
                        // Borrowed from the render context, valid only until the next convert.
                        // Copy into a recycled buffer, and OUTSIDE the slot lock, so a
                        // multi-megabyte memcpy never holds the encoder off the slot.
                        let mut buf = shared.slot.lock().unwrap().take_free().unwrap_or_default();
                        buf.clear();
                        buf.extend_from_slice(data);
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(w as usize, h as usize, fmt, buf);
                        shared.cv.notify_one();
                    }
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(err) => {
                        drm_set_prefer_cpu(&our_key);
                        break format!("convert: {err}");
                    }
                }
                // `recv_fd` closes at the end of this iteration, AFTER convert imported it.
                // Ack so the producer RELEASES ONE SEND CREDIT and forwards the next; this bounds
                // the socket to a couple of in-flight frames instead of a stale backlog.
                if let Err(err) = conn.send_frame_ack().await {
                    break format!("frame ack: {err}");
                }
            }
            Data::DrmFrame { width, height } => {
                // `frame()` hands this to PixelBuffer::new, which derives the stride as
                // `data.len() / height`: height==0 would DIVIDE BY ZERO.
                if width == 0 || height == 0 {
                    break format!("cpu frame: degenerate geometry {width}x{height}");
                }
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                let mut buf = shared.slot.lock().unwrap().take_free().unwrap_or_default();
                match tokio::time::timeout(BODY_READ_TIMEOUT, conn.next_raw_into(&mut buf)).await {
                    Err(_) => break "cpu frame body read timed out".to_owned(),
                    Ok(Ok(())) => {
                        if buf.len() < need {
                            break format!(
                                "cpu frame: body {} bytes < {need} for {width}x{height}",
                                buf.len()
                            );
                        }
                        let mut slot = shared.slot.lock().unwrap();
                        slot.publish(width as usize, height as usize, Pixfmt::BGRA, buf);
                        shared.cv.notify_one();
                    }
                    Ok(Err(err)) => break format!("frame body: {err}"),
                }
                // Ack this CPU frame too (flow control; see the dma-buf arm above).
                if let Err(err) = conn.send_frame_ack().await {
                    break format!("frame ack: {err}");
                }
            }
            Data::DrmCursor {
                id,
                width,
                height,
                hotx,
                hoty,
            } => {
                // get_cursor_data() hands `colors` straight to the client, which renders
                // width*height*4 RGBA bytes: a short body would make it READ PAST THE BUFFER. A
                // hidden-cursor sentinel arrives as 1x1 with a 4-byte body, so `need` is 4 and the
                // check is live.
                let need = (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4);
                let mut raw = Vec::new();
                match tokio::time::timeout(BODY_READ_TIMEOUT, conn.next_raw_into(&mut raw)).await {
                    Err(_) => break "cursor body read timed out".to_owned(),
                    Ok(Ok(())) => {
                        if raw.len() < need {
                            break format!(
                                "cursor body {} bytes < {need} for {width}x{height}",
                                raw.len()
                            );
                        }
                        let t = shared.transform.load(std::sync::atomic::Ordering::Acquire);
                        if t == TRANSFORM_PENDING {
                            pending_cursor = Some((id, width, height, hotx, hoty, raw));
                        } else {
                            pending_cursor = None;
                            deliver_drm_cursor(
                                display,
                                cursor_epoch,
                                id,
                                width,
                                height,
                                hotx,
                                hoty,
                                raw,
                                t,
                            );
                        }
                    }
                    Ok(Err(err)) => break format!("cursor body: {err}"),
                }
            }
            Data::DrmDisplaysChanged(list) => {
                // `display` (the CLIENT's index) and NOT `wire_idx`, deliberately. `bound_to` is an
                // identity `(device, crtc_id)`, not a position, so this asks "does that slot still
                // name MY monitor"; and the swap below installs this list as DRM_STATE, which is the
                // client-space list display_service re-advertises and input is mapped through.
                // Probing `wire_idx` stays quiet in exactly the case this guard exists for: a stream
                // whose wire_idx differs from display keeps running while the client's index comes to
                // mean another monitor. Checked BEFORE the swap, against the topology this stream
                // started on.
                let now_at_our_index = list
                    .get(display.max(0) as usize)
                    .map(|d| (d.device.clone(), d.crtc_id));
                if bound_to.is_some() && now_at_our_index != bound_to {
                    swap_available_displays(list);
                    scrap::wayland::display::clear_wayland_displays_cache();
                    break match (&bound_to, &now_at_our_index) {
                        (Some((_, was)), Some((_, now))) => format!(
                            "hotplug renumbered display {display}: it was crtc {was}, now crtc {now}"
                        ),
                        _ => format!("hotplug removed display {display} from the list"),
                    };
                }
                swap_available_displays(list);
                scrap::wayland::display::clear_wayland_displays_cache();
                UINPUT_REFRESH_GEN.fetch_add(1, Ordering::AcqRel);
                if !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel) {
                    // Taken BEFORE the spawn and moved in: `Builder::spawn` can FAIL with EAGAIN after
                    // the swap, so a guard built inside the closure would never exist and the flag
                    // would stay set for the PROCESS LIFETIME.
                    let mut busy = UinputRefreshGuard(true);
                    let spawned = std::thread::Builder::new()
                        .name("drm-uinput-refresh".into())
                        .spawn(move || {
                        let rt = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt,
                            Err(err) => {
                                log::warn!(
                                    "drm: uinput refresh worker could not build a runtime: {err}"
                                );
                                return; // the guard hands the slot back
                            }
                        };
                        let mut served = 0u64;
                        loop {
                            let g = UINPUT_REFRESH_GEN.load(Ordering::Acquire);
                            if g != served {
                                served = g;
                                rt.block_on(super::super::wayland::update_uinput_resolution());
                                continue;
                            }
                            busy.release();
                            if UINPUT_REFRESH_GEN.load(Ordering::Acquire) == served {
                                break;
                            }
                            if !busy.retake() {
                                break; // another handler already started a fresh worker
                            }
                        }
                    });
                    if let Err(err) = spawned {
                        log::error!("drm: could not spawn the uinput refresh worker: {err}");
                    }
                }
            }
            _ => {} // ignore any unexpected control message
        }
    };
    log::info!("drm capture stream ended: {end_reason}");
    // Drop the render context on THIS thread: its EGL state + cached imports are thread-local and
    // a cross-thread close strands them. Never in `Drop`, which runs on the encoder thread.
    drop(converter);
    remove_drm_cursor(display, cursor_epoch);
    let mut slot = shared.slot.lock().unwrap();
    slot.ended = Some(format!("drm stream ended ({end_reason})"));
    shared.cv.notify_one();
}
