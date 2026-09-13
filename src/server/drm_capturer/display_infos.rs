use super::*;

/// The service holds its answer until the topology settles. Replaces only an `Available` verdict.
pub(in crate::server) async fn refresh_displays_for_login() {
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        if !matches!(&*st, ProbeState::Available(..)) {
            return;
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let t = Instant::now();
    match query_displays_inner().await {
        Ok(list) if !list.is_empty() => {
            let changed = {
                let mut st = DRM_STATE.lock().unwrap();
                if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                    log::debug!(
                        "drm: login display refresh superseded while probing; keeping the newer list"
                    );
                    return;
                }
                match &*st {
                    ProbeState::Available(_, old) => {
                        let changed = *old != list;
                        log::debug!(
                            "drm: login display refresh -> {} display(s) in {:?}{}",
                            list.len(),
                            t.elapsed(),
                            if changed { " (list changed)" } else { "" }
                        );
                        publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                        changed
                    }
                    _ => return,
                }
            };
            if changed {
                scrap::wayland::display::clear_wayland_displays_cache();
            }
        }
        Ok(_) => log::debug!(
            "drm: login display refresh found no displays in {:?}; keeping the cached list",
            t.elapsed()
        ),
        Err(err) => log::debug!(
            "drm: login display refresh failed in {:?} ({err}); keeping the cached list",
            t.elapsed()
        ),
    }
}

/// Mirrors get_display_infos: only a MULTI-display host advertises a demoted display.
pub(in crate::server) fn display_count_and_any_demoted() -> Option<(usize, bool)> {
    // Snapshot the identity keys under DRM_STATE, then consult health with DRM_STATE RELEASED --
    // same order as get_display_infos: never hold DRM_STATE while taking a per-display map.
    let (len, keys): (usize, Vec<String>) = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => (
            list.len(),
            if list.len() > 1 {
                list.iter().map(connector_key).collect()
            } else {
                Vec::new()
            },
        ),
        _ => return None,
    };
    let any_demoted = if len > 1 {
        let health = DRM_DISPLAY_HEALTH.lock().unwrap();
        keys.iter()
            .any(|k| health.get(k).is_some_and(|h| h.demoted()))
    } else {
        false
    };
    Some((len, any_demoted))
}

// A multi-display portal stream cannot replace one demoted connector. Keep its index but mark it
// offline; a single connector remains usable through the whole-desktop fallback - unless that
// fallback itself was rejected on geometry, in which case advertising the lone display online
// would restart-loop the video service against a stream nothing can serve.
pub(super) fn mark_demoted_displays(list: &[DrmDisplayInfo], infos: &mut [DisplayInfo]) {
    let health = DRM_DISPLAY_HEALTH.lock().unwrap();
    if list.len() <= 1 {
        if let (Some(display), Some(info)) = (list.first(), infos.first_mut()) {
            if health
                .get(&connector_key(display))
                .is_some_and(|health| health.demoted() && health.fallback_rejected)
            {
                info.online = false;
            }
        }
        return;
    }
    for (display, info) in list.iter().zip(infos.iter_mut()) {
        if health
            .get(&connector_key(display))
            .is_some_and(|health| health.demoted())
        {
            info.online = false;
        }
    }
}

/// The PipeWire fallback for this display was rejected on geometry; recorded so the lone-display
/// carve-out above stops advertising a display nothing can serve. Cleared by a delivered frame
/// and by the demote-cooldown re-arm.
pub(in crate::server) fn mark_fallback_rejected(display_idx: usize) {
    let Some(expected) = display_info_of(display_idx as i32) else {
        return;
    };
    DRM_DISPLAY_HEALTH
        .lock()
        .unwrap()
        .entry(connector_key(&expected))
        .or_insert_with(DisplayHealth::new)
        .fallback_rejected = true;
}

pub(super) fn primary_index_from_assignment(assignment: &[Option<usize>], primary: usize) -> usize {
    assignment
        .iter()
        .position(|assigned| *assigned == Some(primary))
        .unwrap_or(0)
}

/// Releases DRM_STATE before taking the Wayland and health locks.
pub(in crate::server) fn get_display_infos_and_primary() -> Option<(Vec<DisplayInfo>, usize)> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(&list, &wl.displays);
    let mut infos = augment_with_wayland_geometry_from(&list, &wl, &assignment);
    mark_demoted_displays(&list, &mut infos);
    // Primary and geometry must use the same connector assignment snapshot.
    let primary = primary_index_from_assignment(&assignment, wl.primary);
    Some((infos, primary))
}

pub(in crate::server) fn get_display_infos() -> Option<Vec<DisplayInfo>> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let mut infos = augment_with_wayland_geometry(&list);
    mark_demoted_displays(&list, &mut infos);
    Some(infos)
}
