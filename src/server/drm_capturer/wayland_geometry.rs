use super::*;

/// DRM reports every monitor at physical size and origin (0,0), stacking a multi-monitor client.
///
/// Asked at login screens too, on purpose: a greeter runs a compositor, and the socket fallback in
/// hbb_common lets the enumerator reach it with no environment variables. Where that fallback
/// cannot answer, the list comes back empty and everything stays unaugmented, which is what the
/// old is-login-screen gate produced unconditionally.
pub(super) fn augment_with_wayland_geometry(drm: &[DrmDisplayInfo]) -> Vec<DisplayInfo> {
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    augment_with_wayland_geometry_from(drm, &wl, &assignment)
}

pub(super) fn augment_with_wayland_geometry_from(
    drm: &[DrmDisplayInfo],
    wl: &scrap::wayland::display::Displays,
    matched: &[Option<usize>],
) -> Vec<DisplayInfo> {
    let mut infos: Vec<DisplayInfo> = drm.iter().map(display_info_from_drm).collect();
    // A single display is still augmented: on a multi-GPU host the one connector this service can
    // open may sit at a non-zero origin in the compositor layout, and DRM alone reports (0,0).
    if drm.is_empty() {
        return infos;
    }
    if wl.displays.is_empty() {
        return infos;
    }
    // One connector against one output is the origin-only case: the lone output can still sit at
    // a non-zero origin this side cannot see, but it keeps the scale-1 convention — a single
    // display is advertised at physical size (see `logical_rects_of`), so its logical size must
    // not be adopted. More connectors than the one output is an inconsistent snapshot, and the
    // layout-order fallback in `assign_wayland_outputs` would plant that origin on a guess.
    let origin_only = wl.displays.len() == 1;
    if origin_only && drm.len() > 1 {
        return infos;
    }
    let identity = identity_matches(drm, &wl.displays);
    for (i, info) in infos.iter_mut().enumerate() {
        let Some(w) = matched[i].map(|j| &wl.displays[j]) else {
            continue;
        };
        info.x = w.x;
        info.y = w.y;
        // Rotated size before the origin-only cut: a lone rotated output still delivers rotated
        // frames, so it must advertise them; only the logical-scale adoption stays multi-output.
        // original_resolution follows in the same motion, or the client reads the transposed
        // current size against an untransposed original as a third-party resolution change.
        // Identity matches ONLY, the same rule the capturer's transform follows: swapping on a
        // layout-order guess advertises dimensions the capturer will not deliver.
        let is_identity = identity[i].is_some() && identity[i] == matched[i];
        if is_identity && (w.transform == 90 || w.transform == 270) {
            std::mem::swap(&mut info.width, &mut info.height);
            info.original_resolution = super::super::display_service::get_original_resolution(
                &drm[i].name,
                info.width as usize,
                info.height as usize,
            );
        }
        if origin_only {
            continue;
        }
        if let Some((lw, lh)) = w.logical_size {
            if lw > 0 && lh > 0 {
                // Post-swap width over logical width, which arrives already swapped when rotated:
                // the unrotated numerator made a rotated 1:1 monitor advertise scale 16/9.
                info.scale = info.width as f64 / lw as f64;
                info.original_resolution = super::super::display_service::get_original_resolution(
                    &drm[i].name,
                    lw as usize,
                    lh as usize,
                );
            }
        }
    }
    infos
}

/// Each output goes to at most one connector; unmatched ones take the next free output of the same
/// size, else the next free one in layout order, since leaving them unaugmented keeps them all at
/// DRM's (0,0).
/// The identity half of the assignment (name, or unique resolution), same progressive `taken`
/// as the full one. Rotation keys off THIS on both sides: swapping or turning on a layout-order
/// guess splits the advertised dimensions from the delivered frames.
/// Identity assignment in two GLOBAL passes: every exact name match is reserved first, then
/// resolution pairing runs on the unmatched remainder, and only when it is forced - exactly one
/// free output AND exactly one unmatched connector at that resolution. A resolution guess for an
/// earlier connector must never steal an exact name match from a later one.
pub(super) fn identity_matches(
    drm: &[DrmDisplayInfo],
    wl: &[base::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut taken = vec![false; wl.len()];
    let mut matched: Vec<Option<usize>> = vec![None; drm.len()];
    for (i, d) in drm.iter().enumerate() {
        let dn = normalize_connector(&d.name);
        if let Some((j, _)) = wl
            .iter()
            .enumerate()
            .find(|(j, w)| !taken[*j] && normalize_connector(&w.name) == dn)
        {
            matched[i] = Some(j);
            taken[j] = true;
        }
    }
    for (i, d) in drm.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let free_same: Vec<usize> = wl
            .iter()
            .enumerate()
            .filter(|(j, w)| !taken[*j] && w.width == d.width as i32 && w.height == d.height as i32)
            .map(|(j, _)| j)
            .collect();
        let unmatched_same = drm
            .iter()
            .enumerate()
            .filter(|(k, o)| matched[*k].is_none() && o.width == d.width && o.height == d.height)
            .count();
        if free_same.len() == 1 && unmatched_same == 1 {
            matched[i] = Some(free_same[0]);
            taken[free_same[0]] = true;
        }
    }
    matched
}

pub(super) fn assign_wayland_outputs(
    drm: &[DrmDisplayInfo],
    wl: &[base::platform::linux::WaylandDisplayInfo],
) -> Vec<Option<usize>> {
    let mut matched = identity_matches(drm, wl);
    let mut taken = vec![false; wl.len()];
    for m in matched.iter().flatten() {
        taken[*m] = true;
    }
    for (i, d) in drm.iter().enumerate() {
        if matched[i].is_some() {
            continue;
        }
        let free_same_size = wl
            .iter()
            .enumerate()
            .position(|(j, w)| !taken[j] && w.width == d.width as i32 && w.height == d.height as i32);
        let Some(j) = free_same_size.or_else(|| taken.iter().position(|t| !t)) else {
            continue; // more connectors than outputs; leave the rest unaugmented
        };
        log::warn!(
            "drm: connector {} matched no compositor output by name or by a unique resolution; \
             falling back to layout order and taking {} at ({}, {})",
            d.name,
            wl[j].name,
            wl[j].x,
            wl[j].y
        );
        matched[i] = Some(j);
        taken[j] = true;
    }
    matched
}


/// DRM inserts a single-letter type discriminator the compositor drops ("HDMI-A-1" -> "HDMI-1").
/// Only a *letter* folds: a single *digit* is an MST port index, so "DP-1-2" is not "DP-2".
pub(super) fn normalize_connector(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 3 && parts[1].len() == 1 && parts[1].chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{}-{}", parts[0], parts[2])
    } else {
        name.to_string()
    }
}

pub(super) fn swap_available_displays(list: Vec<DrmDisplayInfo>) {
    let mut st = DRM_STATE.lock().unwrap();
    if matches!(&*st, ProbeState::Available(..)) {
        if list.is_empty() {
            log::info!("drm: hotplug refresh -> 0 displays, marking DRM unavailable");
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
        } else {
            log::info!("drm: hotplug refresh -> {} display(s)", list.len());
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
        }
    }
}
