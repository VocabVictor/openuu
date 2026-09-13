use super::*;

pub(super) fn display_info_from_drm(d: &DrmDisplayInfo) -> DisplayInfo {
    let original_resolution =
        super::super::display_service::get_original_resolution(&d.name, d.width as usize, d.height as usize);
    DisplayInfo {
        x: d.x,
        y: d.y,
        width: d.width as i32,
        height: d.height as i32,
        name: d.name.clone(),
        online: d.active,
        cursor_embedded: false,
        original_resolution,
        scale: 1.0,
        ..Default::default()
    }
}

/// Deliberately does NOT publish the handshake list into DRM_STATE: it is read before a possibly
/// seconds-long stall, and when `wire_idx != display_idx` it is ordered differently.
pub(in crate::server) fn get_capturer_info(
    display_idx: usize,
) -> ResultType<super::super::video_service::CapturerInfo> {
    let expected = display_info_of(display_idx as i32);
    let key = expected.as_ref().map(connector_key);
    {
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        if let Some(h) = key.as_ref().and_then(|k| map.get_mut(k)) {
            if h.zero_frame_streak >= DRM_GRAB_MAX_FAILURES {
                if h.demoted() {
                    bail!(
                        "drm capture for display {display_idx} repeatedly produced no frame; using PipeWire"
                    );
                }
                h.zero_frame_streak = 0;
                h.since = Instant::now();
                // The cooldown re-arms DRM for this display, so the fallback verdict restarts too.
                h.fallback_rejected = false;
            }
        }
    }
    // Built FIRST: a transient `_drm` outage must NOT count toward the flap threshold below.
    let (capturer, displays, wire_idx, origin) = IpcDrmCapturer::new(display_idx as i32, expected)?;
    // The initial build counts 0, so demotion fires on the (RAPID_REBUILD_MAX + 1)-th in a window.
    if let Some(key) = key.clone() {
        let now = Instant::now();
        let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
        let h = map.entry(key).or_insert_with(DisplayHealth::new);
        h.rapid_builds = match h.last_build {
            Some(last) if now.duration_since(last) < RAPID_REBUILD_WINDOW => h.rapid_builds + 1,
            _ => 0,
        };
        h.last_build = Some(now);
        if h.rapid_builds >= RAPID_REBUILD_MAX {
            log::warn!(
                "drm: display {display_idx} rebuilt {} times within {RAPID_REBUILD_WINDOW:?}; flapping, falling back to PipeWire",
                h.rapid_builds
            );
            h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
            h.since = now;
            h.demotes += 1;
            bail!("drm capture for display {display_idx} is flapping; using PipeWire");
        }
    }
    let ndisplay = displays.len();
    // From the entry the stream was BOUND to; `display_idx` is a position in the CLIENT's list.
    let d = displays
        .get(wire_idx)
        .ok_or_else(|| anyhow!("drm display index {wire_idx} out of range ({ndisplay})"))?
        .clone();
    // Origin and transform come from the ONE snapshot new() resolved, so both reflect the
    // same output assignment; dimensions stay PHYSICAL, rotated to frame orientation.
    let origin = origin.unwrap_or((d.x, d.y));
    let (cap_w, cap_h) = rotated_dims(capturer.transform, d.width as usize, d.height as usize);
    Ok(super::super::video_service::CapturerInfo {
        origin,
        width: cap_w,
        height: cap_h,
        ndisplay,
        current: display_idx,
        privacy_mode_id: 0,
        _capturer_privacy_mode_id: 0,
        capturer: Box::new(capturer),
    })
}
