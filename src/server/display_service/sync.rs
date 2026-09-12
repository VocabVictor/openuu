use super::*;

// https://github.com/rustdesk/rustdesk/pull/8537
pub(super) static TEMP_IGNORE_DISPLAYS_CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
pub(super) struct SyncDisplaysInfo {
    pub(super) displays: Vec<DisplayInfo>,
    pub(super) is_synced: bool,
}

impl SyncDisplaysInfo {
    pub(super) fn check_changed(&mut self, displays: &[DisplayInfo]) {
        if self.displays.as_slice() == displays {
            return;
        }

        self.displays = displays.to_vec();
        if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
            self.is_synced = false;
        }
    }

    pub(super) fn get_update_sync_displays(&mut self) -> Option<Vec<DisplayInfo>> {
        if self.is_synced {
            return None;
        }
        self.is_synced = true;
        Some(self.displays.clone())
    }
}

pub fn temp_ignore_displays_changed() -> SimpleCallOnReturn {
    TEMP_IGNORE_DISPLAYS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
    SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            // Wait for a while to make sure check_display_changed() is called
            // after video service has sending its `SwitchDisplay` message(`try_broadcast_display_changed()`).
            std::thread::sleep(Duration::from_millis(1000));
            TEMP_IGNORE_DISPLAYS_CHANGED.store(false, Ordering::Relaxed);
            // Trigger the display changed message.
            SYNC_DISPLAYS.lock().unwrap().is_synced = false;
        }),
    }
}

pub(in crate::server) fn get_sync_displays() -> Vec<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.clone()
}

pub(in crate::server) fn get_display_info(idx: usize) -> Option<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.get(idx).cloned()
}

// True when at least one advertised (synced) display is NOT served by the DRM/KMS capture path,
// i.e. a mixed DRM + PipeWire session. The cursor service (platform::linux::get_cursor /
// get_cursor_data) uses this to decide whether a hidden DRM hardware-cursor sentinel is
// authoritative: in a pure-DRM session it is (the pointer is genuinely off every captured CRTC),
// but in a mixed session the sentinel only means the pointer moved onto a PipeWire-served display,
// whose cursor must come from the normal path instead of being hidden everywhere.
//
// When DRM capture is active the advertised list is enumerated from the DRM display list, so a DRM
// list shorter than the synced list means at least one advertised display is served by PipeWire.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub fn has_non_drm_backed_display() -> bool {
    match super::drm_capturer::display_count_and_any_demoted() {
        // A display served by PipeWire is either ABSENT from the DRM list (a shorter count, e.g. a
        // pure-portal display) or PRESENT-BUT-DEMOTED (kept in place at the same index and marked
        // offline so the index space stays aligned -- see get_display_infos). The count check alone
        // misses the demotion case (same count), so a demoted display is treated as non-DRM-backed
        // too. This is what gates the hidden-cursor sentinel: it stays authoritative only in a
        // pure-DRM session. The scalar accessor is deliberate: this is polled every cursor tick
        // while the sentinel is active, and cloning + geometry-augmenting the whole list per tick
        // (what get_display_infos does) answered the same two facts.
        Some((count, any_demoted)) => {
            count < SYNC_DISPLAYS.lock().unwrap().displays.len() || any_demoted
        }
        None => false,
    }
}

// Display to DisplayInfo
// The DisplayInfo is be sent to the peer.
pub(in crate::server) fn check_update_displays(all: &Vec<Display>) {
    let _ = update_sync_displays(all);
}

/// Whether there is a compositor on this seat worth asking. `get_displays()` does not cache
/// its failure, so where there is none it re-probes every call for an answer that cannot
/// change any caller's outcome. Last in the `&&` chain, so it never runs first on a poll.
#[inline]
#[cfg(target_os = "linux")]
pub(super) fn wayland_has_compositor() -> bool {
    #[cfg(feature = "drm")]
    {
        !crate::platform::linux::is_login_screen_wayland_cached()
    }
    #[cfg(not(feature = "drm"))]
    {
        true
    }
}

// Return the converted input snapshot while updating the shared display cache.
pub(in crate::server) fn update_sync_displays(all: &Vec<Display>) -> Vec<DisplayInfo> {
    // For compatibility: if only one display, scale remains 1.0 and we use the physical size for `uinput`.
    // If there are multiple displays, we use the logical size for `uinput` by setting scale to d.scale().
    #[cfg(target_os = "linux")]
    let use_logical_scale = !is_x11()
        && crate::is_server()
        && wayland_has_compositor()
        && scrap::wayland::display::get_displays().displays.len() > 1;
    let displays = all
        .iter()
        .map(|d| {
            let display_name = d.name();
            #[allow(unused_assignments)]
            #[allow(unused_mut)]
            let mut scale = 1.0;
            #[cfg(target_os = "macos")]
            {
                scale = d.scale();
            }
            #[cfg(target_os = "linux")]
            {
                if use_logical_scale {
                    scale = d.scale();
                }
            }
            let original_resolution = get_original_resolution(
                &display_name,
                ((d.width() as f64) / scale).round() as usize,
                (d.height() as f64 / scale).round() as usize,
            );
            DisplayInfo {
                x: d.origin().0 as _,
                y: d.origin().1 as _,
                width: d.width() as _,
                height: d.height() as _,
                name: display_name,
                online: d.is_online(),
                cursor_embedded: false,
                original_resolution,
                scale,
                ..Default::default()
            }
        })
        .collect::<Vec<DisplayInfo>>();
    SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
    displays
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !is_x11() {
        return super::wayland::is_inited();
    }
    None
}

// Return the primary index with the refreshed list so login cannot mix display snapshots.
pub async fn update_get_sync_displays_on_login() -> ResultType<(Vec<DisplayInfo>, usize)> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            let (displays, primary_display_idx) =
                super::wayland::get_displays_and_primary().await?;
            let primary_display_idx =
                normalize_primary_display_idx(primary_display_idx, displays.len());
            return Ok((displays, primary_display_idx));
        }
    }
    #[cfg(not(windows))]
    let displays = display_service::try_get_displays();
    #[cfg(windows)]
    let displays = display_service::try_get_displays_add_amyuni_headless();
    let displays = displays?;
    let primary_display_idx = get_primary_2(&displays);
    let sync_displays = update_sync_displays(&displays);
    let primary_display_idx =
        normalize_primary_display_idx(primary_display_idx, sync_displays.len());
    Ok((sync_displays, primary_display_idx))
}

#[inline]
pub(super) fn normalize_primary_display_idx(primary_display_idx: usize, display_len: usize) -> usize {
    // Zero is the protocol fallback when the list is empty or its primary index is stale.
    if primary_display_idx < display_len {
        primary_display_idx
    } else {
        0
    }
}

#[inline]
pub fn get_primary_2(all: &Vec<Display>) -> usize {
    all.iter().position(|d| d.is_primary()).unwrap_or(0)
}
