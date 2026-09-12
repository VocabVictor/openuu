use super::*;

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    pub(super) static ref WAYLAND_UINPUT_RECT: Mutex<WaylandUinputRect> = Default::default();
    pub(super) static ref WAYLAND_LAYOUT: Mutex<WaylandLayout> = Default::default();
}

#[cfg(target_os = "linux")]
pub(super) const WAYLAND_LAYOUT_CHECK_INTERVAL: Duration = Duration::from_millis(1500);

#[cfg(target_os = "linux")]
#[derive(Default)]
pub(super) struct WaylandUinputRect {
    pub(super) rect: Option<(i32, i32, i32, i32)>,
    pub(super) last_check: Option<std::time::Instant>,
}

// Per-display layout used to correct injected coordinates when the compositor moves a
// monitor mid-session. The client keeps sending coordinates offset by the layout it was
// told at session init (`baseline`); we remap them onto the current layout (`live`).
// https://github.com/rustdesk/rustdesk/issues/15601
#[cfg(target_os = "linux")]
#[derive(Default)]
pub(super) struct WaylandLayout {
    pub(super) baseline: Vec<scrap::wayland::display::DisplayRect>,
    pub(super) live: Vec<scrap::wayland::display::DisplayRect>,
    // What the live capturers were built against. Separate from `baseline` because a session
    // init resets that one, and the generation detector needs a memory that a reset cannot
    // erase: two inits straddling a rotation would otherwise leave nothing to compare against.
    pub(super) seen: Vec<scrap::wayland::display::DisplayRect>,
    // A capturer recorded a build layout other than `seen`, tagged with the generation it was
    // built at: the poll observed the live layout between that capturer's snapshot read and its
    // record, so one of the two is stale and the next poll owes an edge whatever it sees. Only
    // while that generation is current: the record can also land between the poll consuming an
    // edge and the bump it promotes (or after the bump, with a snapshot from before it), and that
    // capturer rebuilds on its own, so a second promotion would tear the fresh ones down again.
    // Consumed by `observe`, which the poll runs right after `edge`; a session init's baseline
    // reset leaves it alone.
    pub(super) unseen_build: Option<u64>,
}

#[cfg(target_os = "linux")]
impl WaylandLayout {
    // Replace the per-session input baseline. Before the first poll the outgoing baseline is
    // the only record of the layout the capturers were built against, so it seeds `seen`.
    pub(super) fn reset_baseline(&mut self, baseline: Vec<scrap::wayland::display::DisplayRect>) {
        if self.seen.is_empty() {
            let previous = std::mem::take(&mut self.baseline);
            self.seen = previous;
        }
        self.baseline = baseline;
        self.live.clear();
    }

    // An EDGE (live vs the layout the capturers were built against), not a level: comparing
    // against the baseline latches true for the whole session. With nothing observed yet the
    // baseline is that record, and a missing snapshot at init makes the first success the edge,
    // or transform=0 sticks.
    pub(super) fn edge(
        &self,
        live: &[scrap::wayland::display::DisplayRect],
        snapshot_missing: bool,
        generation: u64,
    ) -> bool {
        if self.unseen_build == Some(generation) {
            return true;
        }
        if !self.seen.is_empty() {
            return self.seen != live;
        }
        if self.baseline.is_empty() {
            return snapshot_missing;
        }
        self.baseline != live
    }

    pub(super) fn observe(&mut self, live: &[scrap::wayland::display::DisplayRect]) {
        self.live = live.to_vec();
        self.seen = live.to_vec();
        self.unseen_build = None;
    }

    // What a capturer was built against, which seeds the memory when nothing else has. A session
    // init whose wayland query failed leaves an EMPTY baseline, and the capturer's own retry can
    // then succeed - so the capturer is the only thing that knows the layout it is showing, and
    // without this a rotation before the first poll is invisible to `edge`. Only when empty: a
    // capturer built later must not overwrite the memory the poll is keeping, since on a
    // multi-display session that memory is what the OTHER capturers were built against. A build
    // that disagrees with it is flagged instead: the capturer's snapshot read and this record
    // are two steps, and a poll landing between them observes the live layout first, which
    // would otherwise drop the record and leave the capturer on a transform nothing compares.
    pub(super) fn note_capturer(&mut self, built_on: &[scrap::wayland::display::DisplayRect], built_gen: u64) {
        if built_on.is_empty() {
            return;
        }
        if self.seen.is_empty() {
            self.seen = built_on.to_vec();
        } else if self.seen != built_on {
            // The newest generation wins: a stale record landing late must not hide a fresh one.
            self.unseen_build = Some(self.unseen_build.map_or(built_gen, |g| g.max(built_gen)));
        }
    }
}

// Whether `live` differs from `baseline`. Read on every mouse move, so it is an atomic:
// the common (no-drift) case never touches the layout mutex.
#[cfg(target_os = "linux")]
pub(super) static WAYLAND_LAYOUT_DRIFTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
pub(in crate::server) fn set_wayland_uinput_rect(rect: (i32, i32, i32, i32)) {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
}

// The uinput ABS range currently programmed into the device, for the DRM path's "reapply only when
// it changed" check. The PipeWire path compares it inline in refresh_wayland_uinput_rect_if_changed.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(in crate::server) fn wayland_uinput_rect() -> Option<(i32, i32, i32, i32)> {
    WAYLAND_UINPUT_RECT.lock().unwrap().rect
}

#[cfg(target_os = "linux")]
pub(in crate::server) fn set_wayland_layout_baseline(baseline: Vec<scrap::wayland::display::DisplayRect>) {
    WAYLAND_LAYOUT_DRIFTED.store(false, Ordering::Relaxed);
    WAYLAND_LAYOUT.lock().unwrap().reset_baseline(baseline);
}

/// Record the layout a capturer was just built against, and the snapshot generation it read
/// before taking that layout. See `WaylandLayout::note_capturer`.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(in crate::server) fn note_capturer_layout(
    displays: &[base::platform::linux::WaylandDisplayInfo],
    built_gen: u64,
) {
    if displays.is_empty() {
        return;
    }
    let rects = scrap::wayland::display::logical_rects_of_displays(displays);
    WAYLAND_LAYOUT
        .lock()
        .unwrap()
        .note_capturer(&rects, built_gen);
}

// Remap an injected coordinate onto the live compositor layout when it has drifted from
// what the client was told at session init. Lock-free no-op otherwise.
#[cfg(target_os = "linux")]
pub(in crate::server) fn remap_wayland_uinput_coord(x: i32, y: i32) -> (i32, i32) {
    if !WAYLAND_LAYOUT_DRIFTED.load(Ordering::Relaxed) {
        return (x, y);
    }
    let lock = WAYLAND_LAYOUT.lock().unwrap();
    scrap::wayland::display::remap_to_live_layout(x, y, &lock.baseline, &lock.live)
}

// The uinput absolute range is set when the session inits. If the compositor layout
// changes afterwards (monitor scale/position change, or a portal virtual output
// appearing once the capture starts), injected coordinates get rescaled by the stale
// range and land offset, https://github.com/rustdesk/rustdesk/issues/15601
#[cfg(target_os = "linux")]
pub(super) fn refresh_wayland_uinput_rect_if_changed() {
    if is_x11() || !crate::input_service::wayland_use_uinput() {
        return;
    }
    {
        let mut lock = WAYLAND_UINPUT_RECT.lock().unwrap();
        if let Some(last_check) = lock.last_check {
            if last_check.elapsed() < WAYLAND_LAYOUT_CHECK_INTERVAL {
                return;
            }
        }
        lock.last_check = Some(std::time::Instant::now());
    }
    let Some((rect, live_rects)) = scrap::wayland::display::get_layout_for_uinput_live() else {
        return;
    };
    // Refresh the per-display layout every poll: monitor origins can shift (e.g. two
    // displays swap positions) without changing the overall desktop rect, and the mouse
    // path needs the current per-display geometry to correct coordinates.
    let (live_changed, mut drifted) = {
        let mut layout = WAYLAND_LAYOUT.lock().unwrap();
        #[cfg(feature = "drm")]
        let snapshot_missing = scrap::wayland::display::wayland_snapshot_missing();
        #[cfg(not(feature = "drm"))]
        let snapshot_missing = false;
        #[cfg(feature = "drm")]
        let generation = scrap::wayland::display::wayland_snapshot_generation();
        #[cfg(not(feature = "drm"))]
        let generation = 0;
        let live_changed = layout.edge(&live_rects, snapshot_missing, generation);
        let drifted = !layout.baseline.is_empty()
            && !live_rects.is_empty()
            && layout.baseline != live_rects;
        layout.observe(&live_rects);
        (live_changed, drifted)
    };
    // Single owner of the generation bump: on the cache clear it let every session init tear
    // down every other live capturer. Baseline promotes with the clear (rustdesk#15601).
    #[cfg(feature = "drm")]
    {
        // An edge seen while DRM is transiently non-Available stays OWED rather than consumed.
        static PROMOTION_OWED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        // The latch fires when a capturer was built with no wayland snapshot: a later cache
        // refill makes wayland_snapshot_missing lie, so live_changed alone would miss it. Taken
        // UNCONDITIONALLY: short-circuiting past it on a live_changed poll would leave it set and
        // spend a second, spurious promotion one poll later on the freshly rebuilt capturer.
        let blind_build = super::drm_capturer::take_unrotated_snapshot_pending();
        if live_changed || blind_build {
            PROMOTION_OWED.store(true, Ordering::Release);
        }
        if PROMOTION_OWED.load(Ordering::Acquire) && super::drm_capturer::is_available_cached() {
            PROMOTION_OWED.store(false, Ordering::Release);
            scrap::wayland::display::clear_wayland_displays_cache();
            scrap::wayland::display::bump_layout_generation();
            set_wayland_layout_baseline(live_rects.clone());
            WAYLAND_LAYOUT.lock().unwrap().live = live_rects.clone();
            drifted = false;
        }
    }
    #[cfg(not(feature = "drm"))]
    let _ = live_changed;
    // At a login screen the DRM path owns the rect; only the range/remap update is skipped,
    // the snapshot invalidation above must still run (a greeter session has no other trigger).
    #[cfg(feature = "drm")]
    if crate::platform::linux::is_login_screen_wayland_cached() {
        return;
    }
    // The remap corrects for per-display origin shifts; the uinput ABS range corrects for
    // the overall bounding box. Only enable the remap once the range matches the live
    // layout, otherwise moves would be remapped into a range the device is not yet using.
    // A drift with no bbox change (origins swapped) needs no range update and enables now.
    let mut range_ok = WAYLAND_UINPUT_RECT.lock().unwrap().rect == Some(rect);
    if !range_ok {
        let (minx, maxx, miny, maxy) = rect;
        log::info!(
            "desktop layout changed, update mouse resolution: ({}, {}), ({}, {})",
            minx,
            maxx,
            miny,
            maxy
        );
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => {
                // Bound the IPC wait, this runs on the display service loop and
                // `set_resolution()` has no timeout on the response read.
                // timeout must be built inside the runtime, or it panics
                // "there is no reactor running". See clipboard_service.rs.
                match rt.block_on(async {
                    timeout(
                        3_000,
                        crate::input_service::update_mouse_resolution(minx, maxx, miny, maxy),
                    )
                    .await
                }) {
                    // Record the rect only after a successful apply, so a transient
                    // failure is retried on the next check.
                    Ok(Ok(())) => {
                        WAYLAND_UINPUT_RECT.lock().unwrap().rect = Some(rect);
                        range_ok = true;
                    }
                    Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
                    Err(err) => log::error!("Failed to update mouse resolution: {}", err),
                }
            }
            Err(err) => {
                log::error!("Failed to build tokio runtime: {}", err);
            }
        }
    }
    // Publish the flag last: a `true` read is always backed by a current `live` and a
    // matching uinput range. A failed range apply leaves this false and retries next poll.
    WAYLAND_LAYOUT_DRIFTED.store(drifted && range_ok, Ordering::Relaxed);
}
