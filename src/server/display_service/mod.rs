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

// https://github.com/rustdesk/rustdesk/pull/8537
static TEMP_IGNORE_DISPLAYS_CHANGED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct SyncDisplaysInfo {
    displays: Vec<DisplayInfo>,
    is_synced: bool,
}

impl SyncDisplaysInfo {
    fn check_changed(&mut self, displays: &[DisplayInfo]) {
        if self.displays.as_slice() == displays {
            return;
        }

        self.displays = displays.to_vec();
        if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
            self.is_synced = false;
        }
    }

    fn get_update_sync_displays(&mut self) -> Option<Vec<DisplayInfo>> {
        if self.is_synced {
            return None;
        }
        self.is_synced = true;
        Some(self.displays.clone())
    }
}

pub fn temp_ignore_displays_changed() -> SimpleCallOnReturn {
    TEMP_IGNORE_DISPLAYS_CHANGED.store(true, std::sync::atomic::Ordering::Relaxed);
    SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            // Wait for a while to make sure check_display_changed() is called
            // after video service has sending its `SwitchDisplay` message(`try_broadcast_display_changed()`).
            std::thread::sleep(Duration::from_millis(1000));
            TEMP_IGNORE_DISPLAYS_CHANGED.store(false, Ordering::Relaxed);
            // Trigger the display changed message.
            SYNC_DISPLAYS.lock().unwrap().is_synced = false;
        }),
    }
}

pub(super) fn get_sync_displays() -> Vec<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.clone()
}

pub(super) fn get_display_info(idx: usize) -> Option<DisplayInfo> {
    SYNC_DISPLAYS.lock().unwrap().displays.get(idx).cloned()
}

// True when at least one advertised (synced) display is NOT served by the DRM/KMS capture path,
// i.e. a mixed DRM + PipeWire session. The cursor service (platform::linux::get_cursor /
// get_cursor_data) uses this to decide whether a hidden DRM hardware-cursor sentinel is
// authoritative: in a pure-DRM session it is (the pointer is genuinely off every captured CRTC),
// but in a mixed session the sentinel only means the pointer moved onto a PipeWire-served display,
// whose cursor must come from the normal path instead of being hidden everywhere.
//
// When DRM capture is active the advertised list is enumerated from the DRM display list, so a DRM
// list shorter than the synced list means at least one advertised display is served by PipeWire.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub fn has_non_drm_backed_display() -> bool {
    match super::drm_capturer::display_count_and_any_demoted() {
        // A display served by PipeWire is either ABSENT from the DRM list (a shorter count, e.g. a
        // pure-portal display) or PRESENT-BUT-DEMOTED (kept in place at the same index and marked
        // offline so the index space stays aligned -- see get_display_infos). The count check alone
        // misses the demotion case (same count), so a demoted display is treated as non-DRM-backed
        // too. This is what gates the hidden-cursor sentinel: it stays authoritative only in a
        // pure-DRM session. The scalar accessor is deliberate: this is polled every cursor tick
        // while the sentinel is active, and cloning + geometry-augmenting the whole list per tick
        // (what get_display_infos does) answered the same two facts.
        Some((count, any_demoted)) => {
            count < SYNC_DISPLAYS.lock().unwrap().displays.len() || any_demoted
        }
        None => false,
    }
}

// Display to DisplayInfo
// The DisplayInfo is be sent to the peer.
pub(super) fn check_update_displays(all: &Vec<Display>) {
    let _ = update_sync_displays(all);
}

/// Whether there is a compositor on this seat worth asking. `get_displays()` does not cache
/// its failure, so where there is none it re-probes every call for an answer that cannot
/// change any caller's outcome. Last in the `&&` chain, so it never runs first on a poll.
#[inline]
#[cfg(target_os = "linux")]
fn wayland_has_compositor() -> bool {
    #[cfg(feature = "drm")]
    {
        !crate::platform::linux::is_login_screen_wayland_cached()
    }
    #[cfg(not(feature = "drm"))]
    {
        true
    }
}

// Return the converted input snapshot while updating the shared display cache.
pub(super) fn update_sync_displays(all: &Vec<Display>) -> Vec<DisplayInfo> {
    // For compatibility: if only one display, scale remains 1.0 and we use the physical size for `uinput`.
    // If there are multiple displays, we use the logical size for `uinput` by setting scale to d.scale().
    #[cfg(target_os = "linux")]
    let use_logical_scale = !is_x11()
        && crate::is_server()
        && wayland_has_compositor()
        && scrap::wayland::display::get_displays().displays.len() > 1;
    let displays = all
        .iter()
        .map(|d| {
            let display_name = d.name();
            #[allow(unused_assignments)]
            #[allow(unused_mut)]
            let mut scale = 1.0;
            #[cfg(target_os = "macos")]
            {
                scale = d.scale();
            }
            #[cfg(target_os = "linux")]
            {
                if use_logical_scale {
                    scale = d.scale();
                }
            }
            let original_resolution = get_original_resolution(
                &display_name,
                ((d.width() as f64) / scale).round() as usize,
                (d.height() as f64 / scale).round() as usize,
            );
            DisplayInfo {
                x: d.origin().0 as _,
                y: d.origin().1 as _,
                width: d.width() as _,
                height: d.height() as _,
                name: display_name,
                online: d.is_online(),
                cursor_embedded: false,
                original_resolution,
                scale,
                ..Default::default()
            }
        })
        .collect::<Vec<DisplayInfo>>();
    SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
    displays
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !is_x11() {
        return super::wayland::is_inited();
    }
    None
}

// Return the primary index with the refreshed list so login cannot mix display snapshots.
pub async fn update_get_sync_displays_on_login() -> ResultType<(Vec<DisplayInfo>, usize)> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            let (displays, primary_display_idx) =
                super::wayland::get_displays_and_primary().await?;
            let primary_display_idx =
                normalize_primary_display_idx(primary_display_idx, displays.len());
            return Ok((displays, primary_display_idx));
        }
    }
    #[cfg(not(windows))]
    let displays = display_service::try_get_displays();
    #[cfg(windows)]
    let displays = display_service::try_get_displays_add_amyuni_headless();
    let displays = displays?;
    let primary_display_idx = get_primary_2(&displays);
    let sync_displays = update_sync_displays(&displays);
    let primary_display_idx =
        normalize_primary_display_idx(primary_display_idx, sync_displays.len());
    Ok((sync_displays, primary_display_idx))
}

#[inline]
fn normalize_primary_display_idx(primary_display_idx: usize, display_len: usize) -> usize {
    // Zero is the protocol fallback when the list is empty or its primary index is stale.
    if primary_display_idx < display_len {
        primary_display_idx
    } else {
        0
    }
}

#[inline]
pub fn get_primary_2(all: &Vec<Display>) -> usize {
    all.iter().position(|d| d.is_primary()).unwrap_or(0)
}

#[inline]
#[cfg(windows)]
fn no_displays(displays: &Vec<Display>) -> bool {
    let display_len = displays.len();
    if display_len == 0 {
        true
    } else if display_len == 1 {
        let display = &displays[0];
        if display.width() > DUMMY_DISPLAY_SIDE_MAX_SIZE
            || display.height() > DUMMY_DISPLAY_SIDE_MAX_SIZE
        {
            return false;
        }
        let any_real = crate::platform::resolutions(&display.name())
            .iter()
            .any(|r| {
                (r.height as usize) > DUMMY_DISPLAY_SIDE_MAX_SIZE
                    || (r.width as usize) > DUMMY_DISPLAY_SIDE_MAX_SIZE
            });
        !any_real
    } else {
        false
    }
}

#[inline]
#[cfg(not(windows))]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    Ok(Display::all()?)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays() -> ResultType<Vec<Display>> {
    try_get_displays_(false)
}

// We can't get full control of the virtual display if we use amyuni idd.
// If we add a virtual display, we cannot remove it automatically.
// So when using amyuni idd, we only add a virtual display for headless if it is required.
// eg. when the client is connecting.
#[inline]
#[cfg(windows)]
pub fn try_get_displays_add_amyuni_headless() -> ResultType<Vec<Display>> {
    try_get_displays_(true)
}

#[inline]
#[cfg(windows)]
pub fn try_get_displays_(add_amyuni_headless: bool) -> ResultType<Vec<Display>> {
    let mut displays = Display::all()?;

    // Do not add virtual display if the platform is not installed or the virtual display is not supported.
    if !crate::platform::is_installed() || !virtual_display_manager::is_virtual_display_supported()
    {
        return Ok(displays);
    }

    // Enable headless virtual display when
    // 1. `amyuni` idd is not used.
    // 2. `amyuni` idd is used and `add_amyuni_headless` is true.
    if virtual_display_manager::is_amyuni_idd() && !add_amyuni_headless {
        return Ok(displays);
    }

    // The following code causes a bug.
    // The virtual display cannot be added when there's no session(eg. when exiting from RDP).
    // Because `crate::platform::desktop_changed()` always returns true at that time.
    //
    // The code only solves a rare case:
    // 1. The control side is connecting.
    // 2. The windows session is switching, no displays are detected, but they're there.
    // Then the controlled side plugs in a virtual display for "headless".
    //
    // No need to do the following check. But the code is kept here for marking the issue.
    // If there're someones reporting the issue, we may add a better check by waiting for a while. (switching session).
    // But I don't think it's good to add the timeout check without any issue.
    //
    // If is switching session, no displays may be detected.
    // if displays.is_empty() && crate::platform::desktop_changed() {
    //     return Ok(displays);
    // }

    let no_displays_v = no_displays(&displays);
    if no_displays_v {
        log::debug!("no displays, create virtual display");
        if let Err(e) = virtual_display_manager::plug_in_headless() {
            log::error!("plug in headless failed {}", e);
        } else {
            displays = Display::all()?;
        }
    }
    Ok(displays)
}

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
