use super::*;

/// Uinput desktop rect from the DRM display list, for a login screen where no compositor can be
/// asked. `(minx, maxx, miny, maxy)`, in delivered-orientation physical pixels (a rotated
/// output counts transposed, matching its frames): no compositor here applied a scale, so
/// unlike `desktop_rect_of` there is no logical size to handle.
#[cfg(feature = "drm")]
pub(super) fn drm_desktop_rect_for_uinput() -> Option<(i32, i32, i32, i32)> {
    let displays = super::super::drm_capturer::get_display_infos()?;
    if displays.is_empty() {
        return None;
    }
    let minx = displays.iter().map(|d| d.x).min()?;
    let miny = displays.iter().map(|d| d.y).min()?;
    let maxx = displays.iter().map(|d| d.x + d.width).max()?;
    let maxy = displays.iter().map(|d| d.y + d.height).max()?;
    if maxx <= minx || maxy <= miny {
        return None;
    }
    Some((minx, maxx, miny, maxy))
}

/// Set the uinput absolute-pointer range to the whole logical desktop so the compositor maps
/// injected coordinates 1:1 instead of stretching a single-monitor range across all outputs. The
/// PipeWire path does this inline in `check_init`; the DRM path bypasses check_init so it must do it
/// too, otherwise on a multi-monitor host the injected pointer lands on the wrong output — and the
/// hardware cursor, which lives on whichever CRTC the pointer is over, never appears on the captured
/// CRTC (the "cursor not visible" symptom). Reads the layout from the Wayland outputs, so it is
/// independent of the capture backend.
///
/// This is the DRM path's single copy of what `check_init` does inline for PipeWire, and it does the
/// same three things, for the same reasons:
///
/// - drops the cached Wayland layout first, because it can predate compositor changes made while no
///   session was active (rustdesk#15601), and on the hotplug path it is stale by definition;
/// - bounds the IPC wait, because `uinput::client::set_resolution` reads its reply with no timeout of
///   its own, so a hung uinput socket would otherwise block every video-service start on this branch
///   and wedge the hotplug worker inside `rt.block_on`, leaving `UINPUT_REFRESH_BUSY` latched true so
///   that every later hotplug refresh is silently skipped for the process lifetime;
/// - records the applied rect and snapshots the per-display layout baseline, which is what arms the
///   #15601 drift remap. Without it the remap never activates on the DRM path at all.
///
/// It stays a separate copy rather than being folded into `check_init` because `check_init` ships in
/// every Linux build and this feature must not change the drm-off one by so much as a line.
#[cfg(feature = "drm")]
pub(in crate::server) async fn update_uinput_resolution() {
    if !crate::input_service::wayland_use_uinput() {
        return;
    }
    // Compositor first at a login screen too: a greeter runs one, and the hbb_common socket
    // fallback reaches it with no environment variables. The DRM union is the fallback, and it is
    // a real loss to land there on a multi-monitor host: DRM has no origins, so its union rect
    // mis-maps the pointer whenever the compositor arranged the outputs side by side.
    //
    // Off the executor: the compositor query can block for the socket probe deadline, and this
    // runs on current-thread runtimes (session init and the hotplug worker). The layout baseline
    // is computed in the SAME task: a failed lookup is not cached, so asking for the rects
    // afterwards would rerun the whole socket probe synchronously.
    let (rect, layout) = match hbb_common::tokio::task::spawn_blocking(|| {
        scrap::wayland::display::clear_wayland_displays_cache();
        match scrap::wayland::display::get_desktop_rect_for_uinput() {
            // The lookup above just cached the displays, so the rects come from that snapshot.
            Some(rect) => Some((rect, scrap::wayland::display::get_display_rects_for_uinput())),
            // Raw DRM union: there is no compositor layout to baseline. Empty keeps the #15601
            // remap inactive, which is right when the origins are unknown anyway.
            None => drm_desktop_rect_for_uinput().map(|rect| (rect, Vec::new())),
        }
    })
    .await
    {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            log::warn!("Failed to get desktop rect for uinput");
            return;
        }
        Err(err) => {
            log::warn!("The desktop rect probe task failed: {err}");
            return;
        }
    };
    // Re-snapshot the baseline on every call: this runs at session init and after every hotplug, and
    // the baseline is what the client's coordinates are measured against.
    let snapshot_layout = || {
        super::super::display_service::set_wayland_layout_baseline(layout.clone());
    };
    // Reprogram the device only when the range actually changes. A display stuck in a rebuild loop
    // calls this about once a second, and reapplying an identical range is an IPC roundtrip plus a
    // uinput device reconfiguration under a user who may be at the console.
    if super::super::display_service::wayland_uinput_rect() == Some(rect) {
        snapshot_layout();
        return;
    }
    let (minx, maxx, miny, maxy) = rect;
    log::info!("update mouse resolution: ({minx}, {maxx}), ({miny}, {maxy})");
    match timeout(
        3_000,
        input_service::update_mouse_resolution(minx, maxx, miny, maxy),
    )
    .await
    {
        // Record the rect only after a successful apply, so a transient failure is retried on the
        // next call instead of being remembered as applied.
        Ok(Ok(())) => {
            super::super::display_service::set_wayland_uinput_rect(rect);
            snapshot_layout();
        }
        Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
        Err(err) => log::error!("Failed to update mouse resolution: {}", err),
    }
}
