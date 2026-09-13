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
mod availability;
pub(crate) use availability::*;
mod display_infos;
pub(super) use display_infos::*;
mod wayland_geometry;
use wayland_geometry::*;
mod capturer_info;
pub(super) use capturer_info::*;

/// A delivered frame resets the streak verdicts (`zero_frame_streak`, `demotes`, `since`) and
    /// nothing else.

#[cfg(test)]
mod drm_capturer_tests {
    use super::*;

    mod tests_a;
    use tests_a::*;

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
