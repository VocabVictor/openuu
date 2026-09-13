use super::*;

/// A list index is NOT an identity: `drm_enumerate_all_displays` concatenates per-card lists.
pub(super) fn connector_key(d: &DrmDisplayInfo) -> String {
    format!("{}:{}", d.device, d.name)
}

/// Frame dimensions after undoing `transform` degrees of output rotation.
pub(super) fn rotated_dims(transform: i32, w: usize, h: usize) -> (usize, usize) {
    if transform == 90 || transform == 270 {
        (h, w)
    } else {
        (w, h)
    }
}

/// Hotspot of a rotated cursor bitmap: the same point mapping `unrotate_bgra` applies to
/// pixels, applied to the one coordinate that must keep naming the click point.
pub(super) fn unrotate_hotspot(transform: i32, w: i32, h: i32, hotx: i32, hoty: i32) -> (i32, i32) {
    match transform {
        90 => (h - 1 - hoty, hotx),
        180 => (w - 1 - hotx, h - 1 - hoty),
        270 => (hoty, w - 1 - hotx),
        _ => (hotx, hoty),
    }
}

/// Turn a 4-byte-pixel frame upright into tightly packed `dst`, undoing `transform` degrees;
/// padded `src` rows ok (stride = len/h). Direction pinned by the tests to the measured anchor
/// of rustdesk#15886; libyuv walks pixels, so channel order does not matter.
pub(super) fn unrotate_bgra(src: &[u8], w: usize, h: usize, transform: i32, dst: &mut Vec<u8>) {
    pub(super) const PX: usize = 4;
    let stride = if h > 0 { src.len() / h } else { 0 };
    let (dw, dh) = rotated_dims(transform, w, h);
    dst.resize(
        dw.checked_mul(dh).and_then(|p| p.checked_mul(PX)).unwrap_or(0),
        0,
    );
    if dst.is_empty() || stride < w * PX {
        log::error!("unrotate: rejected geometry {w}x{h} stride {stride}; frame left blank");
        return;
    }
    let mode = match transform {
        90 => scrap::RotationMode::kRotate90,
        180 => scrap::RotationMode::kRotate180,
        270 => scrap::RotationMode::kRotate270,
        _ => scrap::RotationMode::kRotate0,
    };
    unsafe {
        scrap::ARGBRotate(
            src.as_ptr(),
            stride as i32,
            dst.as_mut_ptr(),
            (dw * PX) as i32,
            w as i32,
            h as i32,
            mode,
        );
    }
}

/// Transform and augmented origin for one wire entry, derived from ONE wayland snapshot so both
/// reflect the same output assignment; two `get_displays()` reads could straddle a cache
/// invalidation. `None` origin means nothing to augment with (caller keeps the DRM origin).
pub(super) fn transform_and_origin(
    drm: &[DrmDisplayInfo],
    wire_idx: usize,
    wl: &scrap::wayland::display::Displays,
) -> (i32, Option<(i32, i32)>) {
    if wl.displays.is_empty() || (wl.displays.len() == 1 && drm.len() > 1) {
        if wl.displays.is_empty() && !drm.is_empty() {
            // A later successful enumeration refills the cache and hides this state from
            // wayland_snapshot_missing, so the layout poll needs this durable record to know a
            // capturer was built blind and owes a rebuild.
            UNROTATED_SNAPSHOT_PENDING.store(true, Ordering::Release);
            log::warn!(
                "drm: no wayland snapshot at capturer build for display {:?}; assuming unrotated",
                drm.get(wire_idx).map(|d| d.name.as_str()).unwrap_or("?")
            );
        }
        return (0, None);
    }
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    // The transform comes ONLY from an identity match (name, or unique resolution), through the
    // SAME progressive-taken pass the advertise side keys its swap off: the layout-order
    // fallback is fine for an origin guess, but a rotation pinned on a guess splits the
    // advertised dimensions from the delivered ones.
    let transform = identity_matches(drm, &wl.displays)
        .get(wire_idx)
        .copied()
        .flatten()
        .map(|j| wl.displays[j].transform)
        // Hardware-rotated 180 scans out already upright (i915 advertises rotate-180 and
        // mutter uses it), and wl_output cannot tell hardware from software rotation, so 180
        // keeps master behavior until the plane rotation property travels the wire.
        .map(|t| if t == 90 || t == 270 { t } else { 0 })
        .unwrap_or(0);
    let origin = augment_with_wayland_geometry_from(drm, wl, &assignment)
        .get(wire_idx)
        .map(|di| (di.x, di.y));
    (transform, origin)
}

/// Takes DRM_STATE: never call it while holding one of the per-display maps below.
pub(super) fn display_info_of(display: i32) -> Option<DrmDisplayInfo> {
    match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.get(display.max(0) as usize).cloned(),
        _ => None,
    }
}
