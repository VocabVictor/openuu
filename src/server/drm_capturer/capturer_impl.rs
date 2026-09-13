use super::*;

impl IpcDrmCapturer {
    /// The service resolves indices against ITS OWN enumeration, so the receive thread re-resolves
    /// `expected` by connector identity and returns the index geometry must be read at.
    pub fn new(
        display: i32,
        expected: Option<DrmDisplayInfo>,
    ) -> ResultType<(IpcDrmCapturer, Vec<DrmDisplayInfo>, usize, Option<(i32, i32)>)> {
        let shared = Arc::new(Shared {
            slot: Mutex::new(FrameSlot {
                latest: None,
                free: [None, None],
                ended: None,
            }),
            cv: Condvar::new(),
            transform: std::sync::atomic::AtomicI32::new(TRANSFORM_PENDING),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<ResultType<(Vec<DrmDisplayInfo>, usize)>>();
        {
            let shared = shared.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("drm-recv".into())
                .spawn(move || recv_thread(display, expected, shared, stop, tx))
                .map_err(|err| anyhow!("could not spawn the drm receive thread: {err}"))?;
        }
        let (displays, wire_idx) = match rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS)) {
            Ok(res) => res?,
            Err(_) => {
                // A handshake completing later would stream unowned: Drop never runs here.
                stop.store(true, Ordering::SeqCst);
                bail!("drm capture handshake timed out");
            }
        };
        // One snapshot for the session: transform, origin and the advertised swap must all
        // reflect the same output assignment. The generation is read BEFORE the snapshot, so a
        // clear racing the build rebuilds once instead of running a session on stale geometry.
        let snapshot_gen = scrap::wayland::display::wayland_snapshot_generation();
        let wl = scrap::wayland::display::get_displays();
        let (transform, origin) = transform_and_origin(&displays, wire_idx, &wl);
        // This capturer now shows that layout. If the session init's own wayland query failed it
        // saved an empty baseline, so this is the only record of what the stream is built on.
        super::super::display_service::note_capturer_layout(&wl.displays, snapshot_gen);
        shared
            .transform
            .store(transform, std::sync::atomic::Ordering::Release);
        Ok((
            IpcDrmCapturer {
                shared,
                stop,
                display,
                connector: displays.get(wire_idx).map(connector_key),
                session_size: displays
                    .get(wire_idx)
                    .map(|d| rotated_dims(transform, d.width as usize, d.height as usize)),
                transform,
                snapshot_gen,
                cur: Vec::new(),
                cur_w: 0,
                cur_h: 0,
                cur_fmt: Pixfmt::BGRA,
                got_frame: false,
            },
            displays,
            wire_idx,
            origin,
        ))
    }

    /// Without an identity, skip rather than record under "", which get_capturer_info reads back
    /// as the same key: one unidentifiable display would demote the next.
    pub(super) fn note_session_without_frame(&self) {
        let Some(key) = self.connector.clone() else {
            log::debug!(
                "drm: display {} produced no frame but has no connector identity; \
                 not counting it against any display",
                self.display
            );
            return;
        };
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        let h = map.entry(key).or_insert_with(DisplayHealth::new);
        h.zero_frame_streak += 1;
        h.since = Instant::now();
        if h.zero_frame_streak == DRM_GRAB_MAX_FAILURES {
            h.demotes += 1;
            log::warn!(
                "drm: display {} produced no frame in {} sessions; using PipeWire for it, \
                 retrying DRM in {:?} (demotion {})",
                self.display,
                h.zero_frame_streak,
                demote_cooldown(h.demotes),
                h.demotes
            );
        }
    }
}

impl Drop for IpcDrmCapturer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
