use super::*;

#[derive(Clone, Copy)]
pub(super) struct DisplayHealth {
    pub(super) zero_frame_streak: u32,
    pub(super) since: Instant,
    pub(super) demotes: u32,
    pub(super) last_build: Option<Instant>,
    pub(super) rapid_builds: u32,
    /// The dma-buf convert failed for this display. The COMMON cause is multi-GPU: our render node
    /// is not the GPU that exported the scanout. Follows the monitor for the process run.
    pub(super) prefer_cpu: bool,
    /// The PipeWire fallback for this display was rejected on geometry (a transposed stream), so
    /// the lone-display carve-out in `mark_demoted_displays` must not keep advertising it online.
    pub(super) fallback_rejected: bool,
}

impl DisplayHealth {
    pub(super) fn new() -> Self {
        Self {
            zero_frame_streak: 0,
            since: Instant::now(),
            demotes: 0,
            last_build: None,
            rapid_builds: 0,
            prefer_cpu: false,
            fallback_rejected: false,
        }
    }

    pub(super) fn demoted(&self) -> bool {
        self.zero_frame_streak >= DRM_GRAB_MAX_FAILURES
            && self.since.elapsed() < demote_cooldown(self.demotes)
    }
}

pub(super) static DRM_DISPLAY_HEALTH: Mutex<BTreeMap<String, DisplayHealth>> = Mutex::new(BTreeMap::new());
pub(super) const DRM_GRAB_MAX_FAILURES: u32 = 4;
pub(super) const DEMOTE_COOLDOWN: Duration = Duration::from_secs(30);
pub(super) const DEMOTE_BACKOFF_MAX_SHIFT: u32 = 4;
pub(super) const RAPID_REBUILD_WINDOW: Duration = Duration::from_secs(3);
pub(super) const RAPID_REBUILD_MAX: u32 = 6;

/// Doubling per demotion up to `DEMOTE_BACKOFF_MAX_SHIFT`; a delivered frame zeroes the demote
/// count (see `frame()`), not decayed by time.
pub(super) fn demote_cooldown(demotes: u32) -> Duration {
    DEMOTE_COOLDOWN * (1u32 << demotes.saturating_sub(1).min(DEMOTE_BACKOFF_MAX_SHIFT))
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RefreshOutcome {
    Publish,
    Unavailable,
    Restamp,
    /// The evidence is about the PRODUCER, not the hardware: give the verdict up to `Unknown`.
    GiveUp,
}

/// `failures` counts consecutive failures INCLUDING this one, so it is 1 on the first.
pub(super) fn refresh_outcome(probe: Option<usize>, failures: u32) -> RefreshOutcome {
    match probe {
        Some(0) => RefreshOutcome::Unavailable,
        Some(_) => RefreshOutcome::Publish,
        None if failures >= DRM_REFRESH_MAX_FAILURES => RefreshOutcome::GiveUp,
        None => RefreshOutcome::Restamp,
    }
}

pub(super) fn drm_prefer_cpu(key: &Option<String>) -> bool {
    key.as_ref().is_some_and(|k| {
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .get(k)
            .is_some_and(|h| h.prefer_cpu)
    })
}

pub(super) fn drm_set_prefer_cpu(key: &Option<String>) {
    if let Some(k) = key {
        DRM_DISPLAY_HEALTH
            .lock()
            .unwrap()
            .entry(k.clone())
            .or_insert_with(DisplayHealth::new)
            .prefer_cpu = true;
    }
}

pub(super) fn render_node_count() -> usize {
    std::fs::read_dir("/dev/dri").map_or(0, |entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .and_then(|n| n.strip_prefix("renderD"))
                    .and_then(|minor| minor.parse::<u32>().ok())
                    .is_some()
            })
            .count()
    })
}

pub(super) static UINPUT_REFRESH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// A capturer was built with no wayland snapshot and runs unrotated; the layout poll consumes
/// this to bump the generation once a live snapshot exists.
pub(super) static UNROTATED_SNAPSHOT_PENDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(in crate::server) fn take_unrotated_snapshot_pending() -> bool {
    UNROTATED_SNAPSHOT_PENDING.swap(false, std::sync::atomic::Ordering::AcqRel)
}
pub(super) static UINPUT_REFRESH_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
