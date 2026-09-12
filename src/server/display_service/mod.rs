use super::*;
use crate::common::SimpleCallOnReturn;
#[cfg(target_os = "linux")]
use crate::platform::linux::is_x11;
#[cfg(windows)]
use crate::virtual_display_manager;
#[cfg(windows)]
use hbb_common::get_version_number;
use hbb_common::protobuf::MessageField;
use scrap::Display;
use std::sync::atomic::{AtomicBool, Ordering};

// https://github.com/rustdesk/rustdesk/discussions/6042, avoiding dbus call

pub const NAME: &'static str = "display";

#[cfg(windows)]
const DUMMY_DISPLAY_SIDE_MAX_SIZE: usize = 1024;

struct ChangedResolution {
    original: (i32, i32),
    changed: (i32, i32),
}

lazy_static::lazy_static! {
    static ref IS_CAPTURER_MAGNIFIER_SUPPORTED: bool = is_capturer_mag_supported();
    static ref CHANGED_RESOLUTIONS: Arc<RwLock<HashMap<String, ChangedResolution>>> = Default::default();
    static ref SYNC_DISPLAYS: Arc<Mutex<SyncDisplaysInfo>> = Default::default();
}

#[cfg(target_os = "linux")]
mod wayland_layout;
#[cfg(target_os = "linux")]
pub(super) use wayland_layout::*;
mod resolution;
pub use resolution::*;
mod service_run;
pub use service_run::*;
pub use service_run::new;
mod sync;
pub use sync::*;
mod enumerate;
pub use enumerate::*;

#[cfg(test)]
mod tests {
    use super::normalize_primary_display_idx;

    #[test]
    fn normalize_primary_display_idx_bounds() {
        assert_eq!(normalize_primary_display_idx(0, 0), 0);
        assert_eq!(normalize_primary_display_idx(0, 2), 0);
        assert_eq!(normalize_primary_display_idx(1, 2), 1);
        assert_eq!(normalize_primary_display_idx(2, 2), 0);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod wayland_layout_tests {
    use super::WaylandLayout;
    use scrap::wayland::display::DisplayRect;

    fn layout(w: i32, h: i32, transform: i32) -> Vec<DisplayRect> {
        vec![DisplayRect {
            name: "DP-1".into(),
            x: 0,
            y: 0,
            w,
            h,
            transform,
        }]
    }

    // rustdesk#15886: a video service starts, the output rotates, and a retry starts before the
    // 1.5 s poll. The baseline is reset on both, so it cannot be the edge detector's memory.
    #[test]
    fn a_rotation_between_two_session_inits_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        l.reset_baseline(upright.clone());
        l.reset_baseline(rotated.clone());
        assert!(l.edge(&rotated, false, 0));
    }

    // The same, with no poll ever having run: the outgoing baseline is the only record of what
    // the first capturer was built against.
    #[test]
    fn a_rotation_between_two_inits_before_the_first_poll_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.reset_baseline(rotated.clone());
        assert!(l.edge(&rotated, false, 0));
    }

    // Control: without it the asserts above would pass on a detector that always fires.
    #[test]
    fn repeated_baseline_resets_without_a_rotation_are_not_an_edge() {
        let upright = layout(1920, 1080, 0);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        l.reset_baseline(upright.clone());
        l.reset_baseline(upright.clone());
        assert!(!l.edge(&upright, false, 0));
    }

    // rustdesk#15886: `ensure_inited()` runs the wayland query BEFORE the capturer exists, and a
    // failure there saves an EMPTY baseline. The capturer's own retry can succeed a moment later
    // and build on layout A, and that build is not blind, so nothing else records it. A rotation
    // before the first poll then had no memory to be an edge against.
    #[test]
    fn a_capturer_built_after_a_failed_init_still_owes_a_rebuild() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);

        let mut l = WaylandLayout::default();
        l.reset_baseline(Vec::new());
        l.note_capturer(&upright, 0);
        assert!(l.edge(&rotated, false, 0));

        // The same with another baseline reset between the build and the poll.
        let mut l2 = WaylandLayout::default();
        l2.reset_baseline(Vec::new());
        l2.note_capturer(&upright, 0);
        l2.reset_baseline(rotated.clone());
        assert!(l2.edge(&rotated, false, 0));

        // Control: no rotation, no edge, in both shapes.
        let mut l3 = WaylandLayout::default();
        l3.reset_baseline(Vec::new());
        l3.note_capturer(&upright, 0);
        assert!(!l3.edge(&upright, false, 0));
    }

    // A capturer built while the poll already has a memory must not overwrite it.
    #[test]
    fn a_later_capturer_does_not_overwrite_the_polls_memory() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.observe(&upright);
        l.note_capturer(&rotated, 0);
        assert!(l.edge(&rotated, false, 0), "the poll's memory still says upright");
    }

    // The constructor's snapshot read and its `note_capturer` are two steps, and the poll can
    // land between them. After a failed init (empty baseline) the constructor takes A and
    // publishes it; the output rotates; the poll reads B live, finds nothing recorded and the
    // snapshot present, so no edge, and observes B. The late `note_capturer(A)` then met a
    // non-empty memory and was dropped: the capturer showed A while the detector held B, and B
    // against B never bumped the generation.
    #[test]
    fn a_capturer_record_that_lost_the_race_with_the_first_poll_is_still_an_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(Vec::new());
        assert!(!l.edge(&rotated, false, 0), "nothing recorded and the snapshot is present");
        l.observe(&rotated);
        l.note_capturer(&upright, 0);
        assert!(l.edge(&rotated, false, 0), "the capturer is built on upright, live is rotated");

        // The promotion consumes it: the next poll sees the same layout and stays quiet.
        l.observe(&rotated);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 0));

        // The same with a session init between the late record and the poll.
        let mut l2 = WaylandLayout::default();
        l2.reset_baseline(Vec::new());
        l2.observe(&rotated);
        l2.note_capturer(&upright, 0);
        l2.reset_baseline(rotated.clone());
        assert!(l2.edge(&rotated, false, 0));

        // Control: a late record that agrees with the poll's memory is not an edge.
        let mut l3 = WaylandLayout::default();
        l3.reset_baseline(Vec::new());
        l3.observe(&upright);
        l3.note_capturer(&upright, 0);
        assert!(!l3.edge(&upright, false, 0));
    }

    // The late record can also land after the poll consumed the edge but before the bump that
    // edge promotes, or after the bump with a snapshot taken before it. That capturer is stale
    // by generation and rebuilds on its own, so its record must not buy a second promotion
    // that tears the freshly rebuilt capturers down again.
    #[test]
    fn a_late_record_from_a_generation_already_promoted_is_not_a_second_edge() {
        let upright = layout(1920, 1080, 0);
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(upright.clone());
        l.observe(&upright);
        // The output rotates, the poll consumes the edge, the capturer built on upright at
        // generation 7 records late, and the poll promotes to 8.
        assert!(l.edge(&rotated, false, 7));
        l.observe(&rotated);
        l.note_capturer(&upright, 7);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 8), "the capturer built at 7 rebuilds on its own");

        // Control: a disagreeing record AT the promoted generation is a real edge.
        l.observe(&rotated);
        l.note_capturer(&upright, 8);
        assert!(l.edge(&rotated, false, 8));

        // A stale record landing after a fresh one must not hide the fresh one.
        l.observe(&rotated);
        l.note_capturer(&upright, 8);
        l.note_capturer(&upright, 7);
        assert!(l.edge(&rotated, false, 8));
    }

    // A promotion consumes the edge: the next poll sees the same layout and must stay quiet.
    #[test]
    fn a_promoted_layout_is_not_an_edge_again() {
        let rotated = layout(1080, 1920, 1);
        let mut l = WaylandLayout::default();
        l.reset_baseline(layout(1920, 1080, 0));
        l.observe(&rotated);
        l.reset_baseline(rotated.clone());
        assert!(!l.edge(&rotated, false, 0));
    }
}
