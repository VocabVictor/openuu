use super::*;

pub(in crate::server) async fn get_displays_and_primary() -> ResultType<(Vec<DisplayInfo>, usize)> {
    #[cfg(feature = "drm")]
    if super::super::drm_capturer::is_available_cached() {
        // This function runs once per login (update_get_sync_displays_on_login is its only
        // caller), and login is the moment the client is PROMISED a display list -- so refresh
        // that list over a live `_drm` handshake first. The service wakes sleeping displays and
        // answers with the settled truth, which is what makes an unattended box with an idled,
        // DISABLED panel connectable at all: the cached list would either omit the panel (probed
        // while asleep) or advertise a display with no scanout behind it (probed while awake), and
        // either way the wake then firing inside the capture handshake would change the list the
        // client had already been given. Properly async, so the executor is never blocked; on any
        // failure the cache serves as before.
        super::super::drm_capturer::refresh_displays_for_login().await;
        let snapshot = hbb_common::tokio::task::spawn_blocking(
            super::super::drm_capturer::get_display_infos_and_primary,
        )
        .await
        .map_err(|err| anyhow::anyhow!("Wayland display probe task failed: {err}"))?;
        if let Some(snapshot) = snapshot {
            return Ok(snapshot);
        }
    }
    check_init().await?;
    // Keep one read guard so clear/reinitialization cannot split these across cache snapshots.
    let cap_map = CAP_DISPLAY_INFO.read().unwrap();
    if let Some(addr) = cap_map.values().next() {
        let cap_display_info: *const CapDisplayInfo = *addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            Ok((cap_display_info.displays.clone(), cap_display_info.primary))
        }
    } else {
        bail!("Failed to get capturer display info");
    }
}

pub fn clear() {
    if is_x11() {
        return;
    }
    // The DRM path augments its geometry from the compositor's Wayland outputs (logical origin +
    // scale), which scrap caches process-wide. The PipeWire path clears that cache on session close,
    // but the DRM path opens no PipeWire session, so without this it would keep matching DRM outputs
    // against STALE geometry after a monitor hotplug/rotation/scale change. Invalidate it on teardown
    // so the next session re-reads fresh geometry (lazily, on the next enumeration) and self-heals.
    #[cfg(feature = "drm")]
    if super::super::drm_capturer::is_available_cached() {
        scrap::wayland::display::clear_wayland_displays_cache();
    }
    // NOTE: intentionally do NOT reset the DRM probe cache here. `clear()` runs on every capturer
    // teardown (which happens on each video-service restart), and re-probing `_drm` from the async
    // enumeration path blocks the executor long enough to trip "deadline has elapsed" and spiral
    // into a restart loop. DRM availability is fixed at service start, so the cache stays valid.
    let mut write_lock = CAP_DISPLAY_INFO.write().unwrap();
    for (_, addr) in write_lock.iter() {
        let cap_display_info: *mut CapDisplayInfo = *addr as _;
        unsafe {
            let _box_capturer = Box::from_raw((*cap_display_info).capturer.0);
            let _box_cap_display_info = Box::from_raw(cap_display_info);
        }
    }
    write_lock.clear();

    // Reset PipeWire initialization flag to allow recreation on next init
    *PIPEWIRE_INITIALIZED.write().unwrap() = false;
}

/// Initialize the PipeWire/portal capture path from the plain (sync) video thread, so a DRM display
/// that cannot be captured can fall through to PipeWire for THAT display. `ensure_inited` short-circuits
/// to the DRM branch whenever DRM is globally available, so it never runs `check_init`; this helper
/// drives the same async portal ScreenCast init directly (mirroring `ensure_inited`'s pattern). Needed
/// because `is_available()` is a GLOBAL verdict — it stays true for the still-working DRM outputs — so
/// without a per-display fallback a single failed/demoted DRM display would restart-loop the video
/// service instead of degrading to PipeWire only for itself.
#[cfg(feature = "drm")]
#[tokio::main(flavor = "current_thread")]
pub(super) async fn ensure_pipewire_inited() -> ResultType<()> {
    check_init().await
}
