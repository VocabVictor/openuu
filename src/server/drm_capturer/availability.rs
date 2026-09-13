use super::*;

/// Never probes or blocks. Use in hot paths such as `wayland::clear()`, `is_inited()`, and display
/// enumeration, where seconds of IPC would trip "deadline has elapsed".
pub(crate) fn is_available_cached() -> bool {
    matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..))
}

/// A tri-state assessment of DRM capture availability.
/// `Unsettled` means a probe is in flight or failures have not reached the disable threshold.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
    Available,
    Unavailable,
    Unsettled,
}

/// MAY BLOCK for seconds: never a routing gate, and never on the login request path — that path
/// reads `availability_cached`. This blocking form serves the capture-side callers through
/// `is_available`, where waiting out a settle is acceptable.
pub(super) fn availability() -> Availability {
    let (verdict, stale_no) = {
        let st = DRM_STATE.lock().unwrap();
        // Keep a settled "no" while an off-thread probe re-verifies it, avoiding a transient
        // Unsettled result whenever the negative cache expires.
        let stale_no =
            matches!(&*st, ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL);
        let verdict = match &*st {
            ProbeState::Available(since, _) => {
                Some((Availability::Available, since.elapsed() >= POSITIVE_TTL))
            }
            ProbeState::Unavailable(_) => Some((Availability::Unavailable, false)),
            ProbeState::Unknown => None, // fall through and probe with the lock released
        };
        (verdict, stale_no)
    };
    if let Some((answer, stale)) = verdict {
        if stale {
            refresh_available_async();
        }
        if stale_no {
            refresh_unavailable_async();
        }
        return answer;
    }
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        // Someone else is mid-probe: their result is not in yet, and "not yet" is not "no".
        return match &*DRM_STATE.lock().unwrap() {
            ProbeState::Available(..) => Availability::Available,
            ProbeState::Unavailable(_) => Availability::Unavailable,
            ProbeState::Unknown => Availability::Unsettled,
        };
    }
    let _in_flight = ProbeInFlightGuard;
    probe_and_publish()
}

/// Non-blocking login-path assessment.
/// Unknown starts a probe off-thread; callers require `Available` before admitting a session.
pub(crate) fn availability_cached() -> Availability {
    let (verdict, stale_no) = {
        let st = DRM_STATE.lock().unwrap();
        let stale_no =
            matches!(&*st, ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL);
        let verdict = match &*st {
            ProbeState::Available(since, _) => {
                Some((Availability::Available, since.elapsed() >= POSITIVE_TTL))
            }
            ProbeState::Unavailable(_) => Some((Availability::Unavailable, false)),
            ProbeState::Unknown => None,
        };
        (verdict, stale_no)
    };
    if let Some((answer, stale)) = verdict {
        if stale {
            refresh_available_async();
        }
        if stale_no {
            refresh_unavailable_async();
        }
        return answer;
    }
    if !DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        let in_flight = ProbeInFlightGuard;
        let spawned = std::thread::Builder::new()
            .name("drm-avail-probe".into())
            .spawn(move || {
                let _in_flight = in_flight;
                probe_and_publish();
            });
        // On error the guard moved into the dropped closure and released the flag already.
        if let Err(err) = spawned {
            log::warn!("drm: could not spawn the availability probe thread: {err}");
        }
    }
    Availability::Unsettled
}

/// Probe synchronously and publish the outcome. The caller must hold DRM_PROBE_IN_FLIGHT.
pub(super) fn probe_and_publish() -> Availability {
    let t = Instant::now();
    let result = query_displays();
    let mut st = DRM_STATE.lock().unwrap();
    let answer = match result {
        Ok(list) if !list.is_empty() => {
            log::debug!(
                "drm: availability probe -> available ({} displays) in {:?}",
                list.len(),
                t.elapsed()
            );
            DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
            publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
            Availability::Available
        }
        Ok(_) => {
            log::info!("drm: availability probe -> no displays in {:?}", t.elapsed());
            publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
            Availability::Unavailable
        }
        Err(err) => {
            let n = DRM_PROBE_FAILURES.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= DRM_PROBE_MAX_FAILURES {
                log::info!("drm: availability probe failed {n}x ({err}); disabling DRM");
                publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                Availability::Unavailable
            } else {
                log::info!(
                    "drm: availability probe failed ({err}), attempt {n}/{DRM_PROBE_MAX_FAILURES}; will retry"
                );
                // Deliberately still Unknown in DRM_STATE: this is a retry window, not a verdict.
                Availability::Unsettled
            }
        }
    };
    drop(st);
    answer
}

/// The boolean form for capture-path callers, where an unsettled probe and a definitive "no"
/// route the same way (into the non-DRM fallback).
pub(crate) fn is_available() -> bool {
    availability() == Availability::Available
}

/// The negative mirror of `refresh_available_async`: re-verify a stale Unavailable without ever
/// answering Unknown in the meantime. A failed or empty re-probe re-confirms the "no" with a
/// fresh timestamp; only a non-empty display list flips the verdict.
pub(super) fn refresh_unavailable_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let in_flight = ProbeInFlightGuard;
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        match &*st {
            ProbeState::Unavailable(since) if since.elapsed() >= NEGATIVE_TTL => {}
            _ => return,
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let spawned = std::thread::Builder::new()
        .name("drm-unavail-refresh".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let result = query_displays();
            let mut st = DRM_STATE.lock().unwrap();
            if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                return;
            }
            match result {
                Ok(list) if !list.is_empty() => {
                    log::info!(
                        "drm: availability re-probe -> available ({} displays)",
                        list.len()
                    );
                    DRM_PROBE_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), list));
                    drop(st);
                    scrap::wayland::display::clear_wayland_displays_cache();
                }
                _ => {
                    // Restamp: a failed or empty re-probe is a fresh confirmation of "no".
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
            }
        });
    // Nothing to release on error: the guard moved into the closure and drops with it either way.
    if let Err(err) = spawned {
        log::warn!("drm: could not spawn the unavailability re-probe thread: {err}");
    }
}

pub(super) fn refresh_available_async() {
    if DRM_PROBE_IN_FLIGHT.swap(true, Ordering::AcqRel) {
        return;
    }
    let in_flight = ProbeInFlightGuard;
    let sampled_gen = {
        let st = DRM_STATE.lock().unwrap();
        if !matches!(&*st, ProbeState::Available(..)) {
            return;
        }
        DRM_STATE_GEN.load(Ordering::Acquire)
    };
    let spawned = std::thread::Builder::new()
        .name("drm-avail-refresh".into())
        .spawn(move || {
            let _in_flight = in_flight;
            let result = query_displays();
            let mut st = DRM_STATE.lock().unwrap();
            if DRM_STATE_GEN.load(Ordering::Acquire) != sampled_gen {
                return;
            }
            let failures = match &result {
                Ok(_) => {
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    0
                }
                Err(_) => DRM_REFRESH_FAILURES.fetch_add(1, Ordering::Relaxed) + 1,
            };
            match refresh_outcome(result.as_ref().ok().map(|l| l.len()), failures) {
                RefreshOutcome::Publish => {
                    let fresh = result.unwrap_or_default();
                    let changed = match &*st {
                        ProbeState::Available(_, old) => *old != fresh,
                        _ => true,
                    };
                    publish_probe_state(&mut st, ProbeState::Available(Instant::now(), fresh));
                    if changed {
                        drop(st);
                        scrap::wayland::display::clear_wayland_displays_cache();
                    }
                }
                RefreshOutcome::Unavailable => {
                    log::info!("drm: refresh -> 0 displays, marking DRM unavailable");
                    publish_probe_state(&mut st, ProbeState::Unavailable(Instant::now()));
                }
                // Only the TTL stamp moves, so this does NOT go through publish_probe_state.
                RefreshOutcome::Restamp => {
                    if let ProbeState::Available(since, _) = &mut *st {
                        *since = Instant::now();
                    }
                }
                RefreshOutcome::GiveUp => {
                    log::info!(
                        "drm: availability refresh failed {failures}x ({:?}); the producer looks \
                         gone, dropping the cached verdict so the next enumeration re-probes",
                        result.as_ref().err()
                    );
                    DRM_REFRESH_FAILURES.store(0, Ordering::Relaxed);
                    publish_probe_state(&mut st, ProbeState::Unknown);
                }
            }
        });
    // Nothing to release: the guard moved into the closure and drops with it. Clearing the flag
    // explicitly would let TWO PROBES RUN AT ONCE, since another refresh may already hold it.
    if let Err(err) = spawned {
        log::warn!(
            "drm: could not spawn the availability refresh thread: {err}; the cached verdict \
             stays stale until the next probe"
        );
    }
}

pub(in crate::server) fn warm_availability() {
    // The gate is INSIDE the loop because `get_display_server()` answers "x11" whenever loginctl
    // cannot yet name the seat0 session. `is_x11_for_drm()` is that form minus the greeter
    // blind spot, where plain `is_x11()` is permanently true.
    for _ in 0..10 {
        if crate::platform::linux::is_x11_for_drm() {
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }
        if matches!(&*DRM_STATE.lock().unwrap(), ProbeState::Available(..)) {
            return;
        }
        match query_displays() {
            Ok(list) if !list.is_empty() => {
                log::info!("drm: consumer cache warmed ({} displays) at startup", list.len());
                publish_probe_state(&mut DRM_STATE.lock().unwrap(), ProbeState::Available(Instant::now(), list));
                return;
            }
            _ => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    log::info!("drm: consumer cache warm found no producer at startup (will probe lazily)");
}
