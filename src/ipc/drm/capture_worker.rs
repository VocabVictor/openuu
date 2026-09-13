use super::*;

pub(super) fn drm_capture_worker(
    frame_tx: tokio::sync::mpsc::Sender<DrmProducerMsg>,
    crtc_rx: std::sync::mpsc::Receiver<(String, u32, bool)>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    frames_gated: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    pub(super) const FRAME_INTERVAL: Duration = Duration::from_millis(33);
    // Bound continuous no-frame (WouldBlock) time so a wedged device ends the stream (~5 s).
    pub(super) const MAX_STALLED: u32 = 150;

    let t_conn = std::time::Instant::now();

    // Enumerate FRESH rather than serve the cache: a cached display may no longer be driven.
    let displays = drm_enumerate_settled("a consumer connected");
    if frame_tx
        .blocking_send(DrmProducerMsg::Displays(displays))
        .is_err()
    {
        return;
    }

    let (target_device, target_crtc, need_cpu) = match crtc_rx.recv() {
        Ok(c) => c,
        Err(_) => return,
    };
    let device_arg = if target_device.is_empty() {
        None
    } else {
        Some(target_device.as_str())
    };
    let t_open = std::time::Instant::now();
    let mut reader = match scrap::drm_reader::DrmReader::open(device_arg, target_crtc) {
        Some(r) => r,
        None => {
            log::warn!(
                "drm: failed to open crtc {target_crtc} on {}; closing _drm connection",
                if target_device.is_empty() { "auto" } else { &target_device }
            );
            schedule_drm_cache_refresh();
            return;
        }
    };
    schedule_drm_cache_refresh();
    log::debug!(
        "drm: capture reader for crtc {target_crtc} opened in {:?}",
        t_open.elapsed()
    );

    static DRM_CONN_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let conn_epoch = DRM_CONN_EPOCH.fetch_add(1, Ordering::Relaxed);

    let mut use_dmabuf = !need_cpu;

    let mut last_cursor_id: u64 = 0;
    let mut stalled: u32 = 0;
    let mut logged_first = false;
    while !stop.load(Ordering::Relaxed) {
        let grabbed: Option<std::io::Result<DrmProducerMsg>> = if frames_gated.load(Ordering::Relaxed)
        {
            // `stalled` is left untouched because the device is healthy -- the task bounds this
            // state itself (CREDIT_STALL) since our watchdog cannot advance.
            None
        } else if use_dmabuf {
            Some(match reader.grab_desc() {
                Ok((fd, d)) => Ok(DrmProducerMsg::Frame {
                    desc: DmabufDesc {
                        buffer_id: (d.fb_id as u64) | ((conn_epoch as u64) << 32),
                        width: d.width,
                        height: d.height,
                        format: d.format,
                        modifier: d.modifier,
                        fb_id: d.fb_id,
                        num_planes: d.num_planes,
                        offsets: d.offsets,
                        pitches: d.pitches,
                        hdr_eotf: d.hdr_eotf,
                        hdr_max_nits: d.hdr_max_nits,
                        has_fd: true, // every exported frame carries its fd; see the send below
                    },
                    fd: Some(fd),
                }),
                Err(err) => Err(err),
            })
        } else {
            Some(match reader.grab() {
                Ok((buf, w, h)) => Ok(DrmProducerMsg::FrameCpu {
                    width: w as u32,
                    height: h as u32,
                    data: Bytes::copy_from_slice(buf),
                }),
                Err(err) => Err(err),
            })
        };
        match grabbed {
            None => {}
            Some(Ok(msg)) => {
                stalled = 0;
                if !logged_first {
                    logged_first = true;
                    log::debug!(
                        "drm: first frame for crtc {target_crtc} in {:?} ({} path)",
                        t_conn.elapsed(),
                        if use_dmabuf { "dma-buf" } else { "cpu" }
                    );
                }
                if frame_tx.blocking_send(msg).is_err() {
                    break;
                }
            }
            Some(Err(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                stalled += 1;
                if stalled > MAX_STALLED {
                    log::info!("drm: capture stalled (no frame); closing _drm connection");
                    break;
                }
                std::thread::sleep(FRAME_INTERVAL);
                continue;
            }
            Some(Err(err)) if use_dmabuf && err.kind() == std::io::ErrorKind::Unsupported => {
                log::warn!(
                    "drm: grab_desc unsupported ({err}); switching to CPU-mapped fallback for this connection"
                );
                use_dmabuf = false;
                logged_first = false;
                // The stall counter measured the abandoned path; give the fallback the whole budget.
                stalled = 0;
                continue;
            }
            Some(Err(err)) => {
                log::warn!("drm: capture error: {err}; closing _drm connection");
                break;
            }
        }

        // Ship the cursor shape only when it changes (id is a content hash or the hidden sentinel).
        if let Some(c) = reader.cursor() {
            if c.id != last_cursor_id {
                last_cursor_id = c.id;
                if frame_tx
                    .blocking_send(DrmProducerMsg::Cursor {
                        id: c.id,
                        width: c.width,
                        height: c.height,
                        hotx: c.hotx,
                        hoty: c.hoty,
                        colors: c.colors,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }

        std::thread::sleep(FRAME_INTERVAL);
    }
}
