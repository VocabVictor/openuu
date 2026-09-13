// Unprivileged consumer of the root `--service`'s DRM/KMS capture stream: the service does the
// privileged export (open + grab the scanout dma-buf fd), the EGL detile / RGBA convert runs here.

use crate::ipc::{connect_drm, Data, DrmDisplayInfo};
use hbb_common::{anyhow::anyhow, bail, log, tokio, ResultType};
use base::message_proto::DisplayInfo;
use scrap::drm_render::RenderConverter;
use scrap::drmtap_dl::drmtap_dmabuf_desc;
use scrap::{Frame, Pixfmt, PixelBuffer, TraitCapturer};
use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const HANDSHAKE_TIMEOUT_MS: u64 = 3000;
const DRM_CONNECT_TIMEOUT_MS: u64 = 1000;
/// The service may hold the list back while it wakes sleeping displays: ~3.6s (DRM_WAKE_*).
const DISPLAY_LIST_TIMEOUT_MS: u64 = HANDSHAKE_TIMEOUT_MS + 4000;
/// Covers the connect timeout plus `recv_msg_timeout2` applying DISPLAY_LIST_TIMEOUT_MS TWICE
/// (first byte, then body). The render-node open and the DrmStart send can still overrun it.
const HANDSHAKE_WAIT_MS: u64 = DRM_CONNECT_TIMEOUT_MS + DISPLAY_LIST_TIMEOUT_MS * 2 + 500;
/// Only the header read rechecks `stop`, so bound the body read here rather than relying on
    /// `next_raw_into`'s own cap.
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(5);

mod types;
pub use types::*;
mod geometry;
use geometry::*;
mod health;
pub(super) use health::*;
mod capturer_impl;
mod capturer_frame;
mod recv;
use recv::*;
mod cursor;
pub use cursor::*;
mod probe;
use probe::*;

/// A delivered frame resets the streak verdicts (`zero_frame_streak`, `demotes`, `since`) and
    /// nothing else.

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
fn availability() -> Availability {
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
fn probe_and_publish() -> Availability {
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
fn refresh_unavailable_async() {
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

fn refresh_available_async() {
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

pub(super) fn warm_availability() {
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

/// The service holds its answer until the topology settles. Replaces only an `Available` verdict.
pub(super) async fn refresh_displays_for_login() {
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
pub(super) fn display_count_and_any_demoted() -> Option<(usize, bool)> {
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
fn mark_demoted_displays(list: &[DrmDisplayInfo], infos: &mut [DisplayInfo]) {
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
pub(super) fn mark_fallback_rejected(display_idx: usize) {
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

fn primary_index_from_assignment(assignment: &[Option<usize>], primary: usize) -> usize {
    assignment
        .iter()
        .position(|assigned| *assigned == Some(primary))
        .unwrap_or(0)
}

/// Releases DRM_STATE before taking the Wayland and health locks.
pub(super) fn get_display_infos_and_primary() -> Option<(Vec<DisplayInfo>, usize)> {
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

pub(super) fn get_display_infos() -> Option<Vec<DisplayInfo>> {
    let list = match &*DRM_STATE.lock().unwrap() {
        ProbeState::Available(_, list) => list.clone(),
        _ => return None,
    };
    let mut infos = augment_with_wayland_geometry(&list);
    mark_demoted_displays(&list, &mut infos);
    Some(infos)
}

/// DRM reports every monitor at physical size and origin (0,0), stacking a multi-monitor client.
///
/// Asked at login screens too, on purpose: a greeter runs a compositor, and the socket fallback in
/// hbb_common lets the enumerator reach it with no environment variables. Where that fallback
/// cannot answer, the list comes back empty and everything stays unaugmented, which is what the
/// old is-login-screen gate produced unconditionally.
fn augment_with_wayland_geometry(drm: &[DrmDisplayInfo]) -> Vec<DisplayInfo> {
    let wl = scrap::wayland::display::get_displays();
    let assignment = assign_wayland_outputs(drm, &wl.displays);
    augment_with_wayland_geometry_from(drm, &wl, &assignment)
}

fn augment_with_wayland_geometry_from(
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
            info.original_resolution = super::display_service::get_original_resolution(
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
                info.original_resolution = super::display_service::get_original_resolution(
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
fn identity_matches(
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

fn assign_wayland_outputs(
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
fn normalize_connector(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 3 && parts[1].len() == 1 && parts[1].chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{}-{}", parts[0], parts[2])
    } else {
        name.to_string()
    }
}

fn swap_available_displays(list: Vec<DrmDisplayInfo>) {
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

fn display_info_from_drm(d: &DrmDisplayInfo) -> DisplayInfo {
    let original_resolution =
        super::display_service::get_original_resolution(&d.name, d.width as usize, d.height as usize);
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
pub(super) fn get_capturer_info(
    display_idx: usize,
) -> ResultType<super::video_service::CapturerInfo> {
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
    Ok(super::video_service::CapturerInfo {
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

#[cfg(test)]
mod drm_capturer_tests {
    use super::*;

    fn capturer_with(session: Option<(usize, usize)>) -> IpcDrmCapturer {
        capturer_named(session, None)
    }

    // DRM_DISPLAY_HEALTH is process-wide and tests run in parallel: pass each test its OWN key.
    fn capturer_named(session: Option<(usize, usize)>, key: Option<&str>) -> IpcDrmCapturer {
        let connector = key.map(|k| k.to_owned());
        IpcDrmCapturer {
            shared: Arc::new(Shared {
                slot: Mutex::new(FrameSlot {
                    latest: None,
                    free: [None, None],
                    ended: None,
                }),
                cv: Condvar::new(),
                transform: std::sync::atomic::AtomicI32::new(0),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            display: 0,
            connector,
            session_size: session,
            transform: 0,
            snapshot_gen: scrap::wayland::display::wayland_snapshot_generation(),
            cur: Vec::new(),
            cur_w: 0,
            cur_h: 0,
            cur_fmt: Pixfmt::BGRA,
            got_frame: false,
        }
    }

    /// One BGRA pixel per label byte, so a rotation result reads as a matrix of labels.
    fn px_frame(labels: &[&[u8]], pad_bytes: usize) -> (Vec<u8>, usize, usize) {
        let h = labels.len();
        let w = labels[0].len();
        let mut buf = Vec::new();
        for row in labels {
            for &l in *row {
                buf.extend_from_slice(&[l, l, l, 255]);
            }
            buf.extend(std::iter::repeat(0u8).take(pad_bytes));
        }
        (buf, w, h)
    }

    fn labels_of(buf: &[u8], w: usize, h: usize) -> Vec<Vec<u8>> {
        (0..h)
            .map(|y| (0..w).map(|x| buf[(y * w + x) * 4]).collect())
            .collect()
    }

    #[test]
    fn a_lone_display_goes_offline_only_when_its_fallback_was_rejected() {
        // Unique name = unique health key; DRM_DISPLAY_HEALTH is process-wide.
        let list = vec![drm_display("TEST-lone-fallback", 1080, 1920)];
        let key = connector_key(&list[0]);
        let demoted = DisplayHealth {
            zero_frame_streak: DRM_GRAB_MAX_FAILURES,
            demotes: 1,
            ..DisplayHealth::new()
        };
        // Demoted alone keeps the lone display online: the whole-desktop fallback is usable.
        DRM_DISPLAY_HEALTH.lock().unwrap().insert(key.clone(), demoted);
        let mut infos = vec![DisplayInfo {
            online: true,
            ..Default::default()
        }];
        mark_demoted_displays(&list, &mut infos);
        assert!(infos[0].online, "the lone-display carve-out must survive");
        // A rejected fallback ends the carve-out: advertising online would restart-loop.
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get_mut(&key)
            .expect("just inserted")
            .fallback_rejected = true;
        mark_demoted_displays(&list, &mut infos);
        assert!(!infos[0].online, "a rejected fallback must take the lone display offline");
        // Once the demotion cooldown lapses the display is no longer demoted, and online returns
        // even with the rejection still latched (the re-arm will clear it on the next build).
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get_mut(&key)
            .expect("still there")
            .since = Instant::now() - demote_cooldown(1) - Duration::from_secs(1);
        infos[0].online = true;
        mark_demoted_displays(&list, &mut infos);
        assert!(infos[0].online, "past the cooldown the verdict is DRM's to retry");
    }

    #[test]
    fn the_cursor_id_names_the_orientation_too() {
        // Same wire cursor under two transforms must publish as two ids, or the client's by-id
        // cache serves the previous orientation after a mid-session rotation.
        let wire = 0xDEAD_BEEF_u64;
        assert_ne!(fold_cursor_id(wire, 0), fold_cursor_id(wire, 90));
        assert_ne!(fold_cursor_id(wire, 90), fold_cursor_id(wire, 270));
        // Deterministic per (id, transform), so an unchanged cursor is still deduped.
        assert_eq!(fold_cursor_id(wire, 90), fold_cursor_id(wire, 90));
        // The hidden sentinel is compared by VALUE at the consumers, so it must pass unfolded.
        let hidden = scrap::drm_reader::HIDDEN_CURSOR_ID;
        assert_eq!(fold_cursor_id(hidden, 90), hidden);
    }

    #[test]
    fn unrotate_hotspot_follows_the_pixel_mapping() {
        // 3 wide x 2 tall, hotspot at (2,0) (top-right): after the 90 turn (left column to top
        // row) that pixel sits at (1,2) in the 2x3 result; 270 sends it to (0,0).
        assert_eq!(unrotate_hotspot(90, 3, 2, 2, 0), (1, 2));
        assert_eq!(unrotate_hotspot(270, 3, 2, 2, 0), (0, 0));
        assert_eq!(unrotate_hotspot(180, 3, 2, 2, 0), (0, 1));
        assert_eq!(unrotate_hotspot(0, 3, 2, 2, 0), (2, 0));
    }

    #[test]
    fn a_stale_snapshot_generation_asks_for_a_rebuild_without_blaming_the_display() {
        let mut c = capturer_named(Some((64, 32)), Some("test:gen-rebuild"));
        c.snapshot_gen = c.snapshot_gen.wrapping_sub(1);
        put_frame(&c, 64, 32);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a stale generation must rebuild, not deliver"),
        };
        assert!(err.to_string().contains("layout changed"), "{err}");
        assert!(!c.got_frame);
        assert_eq!(
            zero_frame_streak_of(&c),
            0,
            "a layout rebuild must not count against display health"
        );
    }

    #[test]
    fn unrotate_90_maps_the_left_column_to_the_top_row() {
        // The measured anchor from rustdesk#15886: mutter transform=1 carries the panel bar down
        // the scanout's LEFT edge, and upright means that edge becomes the TOP row.
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut dst);
        // src left column top-to-bottom = [1, 4]; clockwise puts it on the top row as [4, 1].
        assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
    }

    #[test]
    fn unrotate_270_is_the_inverse_of_90() {
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut once = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut once);
        let mut back = Vec::new();
        unrotate_bgra(&once, h, w, 270, &mut back);
        assert_eq!(back, src);
    }

    #[test]
    fn unrotate_180_reverses_both_axes() {
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 180, &mut dst);
        assert_eq!(labels_of(&dst, w, h), vec![vec![6, 5, 4], vec![3, 2, 1]]);
    }

    #[test]
    fn unrotate_reads_padded_strides_and_writes_tight() {
        // Row stride is derived from len/h, so a padded source must not shear the result.
        let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 8);
        let mut dst = Vec::new();
        unrotate_bgra(&src, w, h, 90, &mut dst);
        assert_eq!(dst.len(), w * h * 4);
        assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
        let mut plain = Vec::new();
        unrotate_bgra(&src, w, h, 0, &mut plain);
        assert_eq!(labels_of(&plain, w, h), vec![vec![1, 2, 3], vec![4, 5, 6]]);
    }

    #[test]
    fn a_rotated_session_delivers_rotated_frames_and_guards_in_rotated_dims() {
        use scrap::TraitPixelBuffer;
        let mut c = capturer_with(Some((32, 64))); // rotated session of a 64x32 scanout
        c.transform = 90;
        put_frame(&c, 64, 32);
        match c.frame(Duration::from_millis(50)) {
            Ok(Frame::PixelBuffer(pb)) => {
                assert_eq!((pb.width(), pb.height()), (32, 64));
            }
            Ok(_) => panic!("expected a pixel-buffer frame"),
            Err(err) => panic!("expected a delivered frame, got {err}"),
        }
        // A scanout change still ends the session, reported in rotated dimensions.
        put_frame(&c, 32, 64);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a scanout change must end a rotated session too"),
        };
        assert!(err.to_string().contains("(32x64 -> 64x32)"), "{err}");
    }

    fn zero_frame_streak_of(c: &IpcDrmCapturer) -> u32 {
        let key = c.connector.clone().expect("this check needs an identity");
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get(&key)
            .map(|h| h.zero_frame_streak)
            .unwrap_or(0)
    }

    fn put_frame(c: &IpcDrmCapturer, w: usize, h: usize) {
        let mut buf = c.shared.slot.lock().unwrap().take_free().unwrap_or_default();
        buf.clear();
        buf.resize(w * h * 4, 0);
        let mut slot = c.shared.slot.lock().unwrap();
        slot.publish(w, h, Pixfmt::BGRA, buf);
    }

    #[test]
    fn a_delivered_frame_clears_the_streak_but_keeps_the_cadence_and_the_convert_verdict() {
        let key = "test:frame-keeps-cadence";
        let mut c = capturer_named(Some((64, 32)), Some(key));
        {
            let mut map = DRM_DISPLAY_HEALTH.lock().unwrap();
            let h = map.entry(key.to_owned()).or_insert_with(DisplayHealth::new);
            h.zero_frame_streak = 2;
            h.demotes = 1;
            h.rapid_builds = 3;
            h.last_build = Some(Instant::now());
            h.prefer_cpu = true;
            h.fallback_rejected = true;
        }
        put_frame(&c, 64, 32);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));

        // Copy out and RELEASE the guard before asserting: a failing assertion while holding
        // process-wide DRM_DISPLAY_HEALTH poisons the mutex for every sibling test.
        let h = {
            let map = DRM_DISPLAY_HEALTH.lock().unwrap();
            *map.get(key).expect("the entry must SURVIVE a delivered frame")
        };
        assert_eq!(h.zero_frame_streak, 0, "a delivered frame refutes the zero-frame streak");
        assert_eq!(h.demotes, 0, "and the demotion count that streak drove");
        assert!(
            !h.fallback_rejected,
            "a delivered frame also refutes the rejected-fallback verdict"
        );
        assert_eq!(
            h.rapid_builds, 3,
            "but it says NOTHING about the rebuild cadence: keeping it is what lets the flap guard \
             reach RAPID_REBUILD_MAX for a display that delivers a first frame and then fails"
        );
        assert!(h.last_build.is_some(), "same for the timestamp the cadence is measured from");
        assert!(
            h.prefer_cpu,
            "and nothing about which GPU exports the scanout: only a topology change may clear it"
        );
    }

    #[test]
    fn frame_of_the_session_size_is_delivered() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        assert!(
            matches!(c.frame(Duration::from_millis(50)), Ok(_)),
            "a frame matching the session geometry must be delivered"
        );
        assert!(c.got_frame);
    }

    #[test]
    fn a_smaller_frame_ends_the_session_instead_of_being_encoded() {
        let mut c = capturer_named(Some((1920, 1080)), Some("test:mid-session-shrink"));
        put_frame(&c, 1920, 1080);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        put_frame(&c, 1280, 720);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a mid-session shrink must be a hard error, not a delivered frame"),
        };
        assert!(err.to_string().contains("changed geometry mid-session"));
        assert!(
            c.got_frame,
            "the rebuild must not look like a display that never produced a frame"
        );
        assert_eq!(
            zero_frame_streak_of(&c),
            0,
            "a session that streamed must not be counted as one that produced nothing"
        );
    }

    #[test]
    fn a_first_frame_that_never_matched_counts_as_a_session_without_frames() {
        let mut c = capturer_named(Some((1920, 1080)), Some("test:never-matched"));
        put_frame(&c, 1280, 720);
        let err = match c.frame(Duration::from_millis(50)) {
            Err(e) => e,
            Ok(_) => panic!("a first frame off the advertised geometry must be a hard error"),
        };
        assert!(err.to_string().contains("never matched its advertised geometry"));
        assert!(!c.got_frame, "no frame reached the encoder, so none was produced");
        assert_eq!(
            zero_frame_streak_of(&c),
            1,
            "the display must be on its way to a PipeWire demotion, not just rebuilding"
        );
    }

    #[test]
    fn a_larger_frame_ends_the_session_too() {
        let mut c = capturer_with(Some((1280, 720)));
        put_frame(&c, 1920, 1080);
        assert!(matches!(c.frame(Duration::from_millis(50)), Err(_)));
    }

    #[test]
    fn unknown_session_size_delivers_whatever_arrives() {
        let mut c = capturer_with(None);
        put_frame(&c, 800, 600);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
    }

    fn drm_display(name: &str, w: u32, h: u32) -> DrmDisplayInfo {
        DrmDisplayInfo {
            name: name.to_owned(),
            crtc_id: 1,
            x: 0,
            y: 0,
            width: w,
            height: h,
            active: true,
            render_node: String::new(),
            device: String::new(),
        }
    }

    fn wl_display(
        name: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> base::platform::linux::WaylandDisplayInfo {
        base::platform::linux::WaylandDisplayInfo {
            name: name.to_owned(),
            x,
            y,
            width: w,
            height: h,
            logical_size: Some((w, h)),
            refresh_rate: 60,
            transform: 0,
        }
    }

    #[test]
    fn a_lone_rotated_output_advertises_delivered_dimensions() {
        // Fix for the origin-only cut: one connector, one rotated output. The capturer will
        // deliver rotated frames, so the advertised size must swap even in the origin-only case,
        // while the logical scale is still not adopted (stays 1.0).
        let drm = [drm_display("HDMI-A-1", 1920, 1080)];
        let mut out = wl_display("HDMI-1", 0, 0, 1920, 1080);
        out.transform = 90;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![out],
        };
        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].width, infos[0].height), (1080, 1920));
        assert_eq!(infos[0].scale, 1.0);
    }

    #[test]
    fn transform_and_origin_come_from_the_same_snapshot() {
        // Both derive from ONE Displays snapshot: the rotated output's transform and its origin
        // must belong to the same assignment, and the multi-connector one-output guard zeroes
        // both rather than mixing a guessed origin with a real transform.
        let drm = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        let mut rotated = wl_display("DP-1", 1920, 0, 2560, 1440);
        rotated.transform = 270;
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![rotated, wl_display("HDMI-1", 0, 0, 1920, 1080)],
        };
        let (t, origin) = transform_and_origin(&drm, 1, &wl);
        assert_eq!(t, 270);
        assert_eq!(origin, Some((1920, 0)));
        let lone = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![wl_display("HDMI-1", 0, 0, 1920, 1080)],
        };
        assert_eq!(transform_and_origin(&drm, 1, &lone), (0, None));
    }

    #[test]
    fn one_connector_assignment_drives_geometry_and_primary() {
        let drm = [
            drm_display("HDMI-A-1", 1920, 1080),
            drm_display("DP-1", 2560, 1440),
        ];
        let wl = scrap::wayland::display::Displays {
            primary: 0,
            displays: vec![
                wl_display("DP-1", 1920, 0, 2560, 1440),
                wl_display("HDMI-1", 0, 0, 1920, 1080),
            ],
        };

        let assignment = assign_wayland_outputs(&drm, &wl.displays);
        let infos = augment_with_wayland_geometry_from(&drm, &wl, &assignment);
        assert_eq!((infos[0].x, infos[1].x), (0, 1920));
        assert_eq!(primary_index_from_assignment(&assignment, wl.primary), 1);
    }

    #[test]
    fn frame_buffers_circulate_instead_of_being_reallocated() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        put_frame(&c, 64, 32);
        let recycled = c
            .shared
            .slot
            .lock()
            .unwrap()
            .free
            .iter()
            .find_map(|b| b.as_ref())
            .map(|b| b.as_ptr());
        assert!(
            recycled.is_some(),
            "a superseded frame must be handed back, not dropped"
        );
        put_frame(&c, 64, 32);
        assert_eq!(
            c.shared
                .slot
                .lock()
                .unwrap()
                .latest
                .as_ref()
                .map(|(.., b)| b.as_ptr()),
            recycled,
            "the receive path must refill the recycled buffer rather than allocate"
        );
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        assert!(
            c.shared.slot.lock().unwrap().free.iter().any(|b| b.is_some()),
            "the buffer the encoder finished with must be handed back to the receive path"
        );
    }

    // Against a single free slot this asserts red: counting the offers is the point.
    #[test]
    fn two_idle_buffers_are_both_kept_rather_than_one_being_dropped() {
        let mut c = capturer_with(Some((64, 32)));
        put_frame(&c, 64, 32);
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        while c.shared.slot.lock().unwrap().take_free().is_some() {}

        put_frame(&c, 64, 32); // fills a fresh buffer (nothing on offer) and publishes it
        put_frame(&c, 64, 32); // supersedes it -> deposit #1
        assert_eq!(
            c.shared.slot.lock().unwrap().free.iter().flatten().count(),
            1,
            "the superseded frame is the first idle buffer"
        );
        assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
        assert_eq!(
            c.shared.slot.lock().unwrap().free.iter().flatten().count(),
            2,
            "both idle buffers must be kept; a single slot dropped the older one"
        );
    }

    #[test]
    fn a_resolution_guess_never_steals_an_exact_name_match() {
        // The review's scenario: an earlier connector with an unmatchable name shares the
        // resolution of a later connector's exact name match. Names reserve globally first.
        let drm = vec![
            drm_display("DSI-1", 1920, 1080),
            drm_display("HDMI-A-1", 1920, 1080),
        ];
        let wl = vec![
            wl_display("HDMI-1", 0, 0, 1920, 1080),
            wl_display("Unknown-9", 1920, 0, 2560, 1440),
        ];
        let m = identity_matches(&drm, &wl);
        assert_eq!(m[1], Some(0), "the exact name match must win globally");
        assert_eq!(m[0], None, "the leftover pairing is not forced, so no identity");
        // Two unmatched connectors at the lone free resolution: ambiguous on the DRM side too,
        // so rotation must not be pinned on either.
        let drm2 = vec![
            drm_display("DSI-1", 1920, 1080),
            drm_display("DSI-2", 1920, 1080),
        ];
        let wl2 = vec![wl_display("HDMI-1", 0, 0, 1920, 1080)];
        let m2 = identity_matches(&drm2, &wl2);
        assert!(m2[0].is_none() && m2[1].is_none());
    }

    #[test]
    fn outputs_are_matched_by_name_across_the_drm_naming_difference() {
        let drm = [drm_display("HDMI-A-1", 1920, 1080), drm_display("DP-1", 2560, 1440)];
        let wl = [wl_display("DP-1", 1920, 0, 2560, 1440), wl_display("HDMI-1", 0, 0, 1920, 1080)];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(1), Some(0)]);
    }

    // The M10 case: same model and resolution, names that do not normalize to the compositor's.
    #[test]
    fn identical_monitors_that_match_no_name_take_layout_order() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("DP-2", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    #[test]
    fn one_output_is_never_claimed_by_two_connectors() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("DP-2", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 3840, 2160),
        ];
        let got = assign_wayland_outputs(&drm, &wl);
        assert_eq!(got[0], Some(0));
        assert_ne!(got[0], got[1], "two connectors must not share one output");
    }

    #[test]
    fn a_name_match_beats_the_positional_fallback() {
        let drm = [drm_display("DP-1", 1920, 1080), drm_display("HDMI-A-1", 1920, 1080)];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("HDMI-1", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1)]);
    }

    #[test]
    fn extra_connectors_stay_unmatched() {
        let drm = [
            drm_display("DP-1", 1920, 1080),
            drm_display("DP-2", 1920, 1080),
            drm_display("DP-3", 1920, 1080),
        ];
        let wl = [
            wl_display("Unknown-1", 0, 0, 1920, 1080),
            wl_display("Unknown-2", 1920, 0, 1920, 1080),
        ];
        assert_eq!(assign_wayland_outputs(&drm, &wl), vec![Some(0), Some(1), None]);
    }

    #[test]
    fn refresh_keeps_a_verdict_through_one_failure_and_gives_it_up_after_a_run() {
        assert_eq!(refresh_outcome(Some(3), 0), RefreshOutcome::Publish);
        assert_eq!(refresh_outcome(Some(1), 0), RefreshOutcome::Publish);
        assert_eq!(refresh_outcome(Some(0), 0), RefreshOutcome::Unavailable);
        assert_eq!(refresh_outcome(None, 1), RefreshOutcome::Restamp);
        assert_eq!(
            refresh_outcome(None, DRM_REFRESH_MAX_FAILURES - 1),
            RefreshOutcome::Restamp
        );
        assert_eq!(
            refresh_outcome(None, DRM_REFRESH_MAX_FAILURES),
            RefreshOutcome::GiveUp
        );
        assert_eq!(
            refresh_outcome(None, DRM_REFRESH_MAX_FAILURES + 5),
            RefreshOutcome::GiveUp
        );
    }

    #[test]
    fn a_dead_producer_stops_being_advertised() {
        let mut outcome = RefreshOutcome::Restamp;
        for failures in 1..=DRM_REFRESH_MAX_FAILURES {
            outcome = refresh_outcome(None, failures);
        }
        assert_eq!(outcome, RefreshOutcome::GiveUp);
        assert!(
            DRM_REFRESH_MAX_FAILURES >= 2,
            "a single transient failure must never be enough to drop the verdict"
        );
    }

    #[test]
    fn health_reports_demoted_only_while_the_cooldown_runs() {
        let mut h = DisplayHealth::new();
        assert!(!h.demoted(), "a fresh display is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES - 1;
        assert!(!h.demoted(), "one session short of the threshold is not demoted");
        h.zero_frame_streak = DRM_GRAB_MAX_FAILURES;
        h.demotes = 1;
        assert!(h.demoted(), "at the threshold, inside the cooldown");
        h.since = Instant::now() - demote_cooldown(h.demotes) - Duration::from_secs(1);
        assert!(!h.demoted(), "past the cooldown the display must be retried");
        h.demotes = 4;
        assert!(h.demoted(), "the backoff must still be holding it at demotion 4");
    }

    #[test]
    fn demote_cooldown_doubles_per_cycle_and_caps() {
        assert_eq!(demote_cooldown(1), DEMOTE_COOLDOWN);
        assert_eq!(demote_cooldown(2), DEMOTE_COOLDOWN * 2);
        assert_eq!(demote_cooldown(3), DEMOTE_COOLDOWN * 4);
        let cap = DEMOTE_COOLDOWN * (1 << DEMOTE_BACKOFF_MAX_SHIFT);
        assert_eq!(demote_cooldown(1 + DEMOTE_BACKOFF_MAX_SHIFT), cap);
        assert_eq!(demote_cooldown(50), cap);
        assert_eq!(demote_cooldown(u32::MAX), cap);
        assert_eq!(demote_cooldown(0), DEMOTE_COOLDOWN);
    }

    #[test]
    fn a_permanently_ungrabbable_display_stops_churning() {
        let burn = Duration::from_secs(5); // four failed sessions
        assert!(demote_cooldown(1) + burn < Duration::from_secs(40));
        assert!(demote_cooldown(5) + burn > Duration::from_secs(8 * 60));
    }
}
