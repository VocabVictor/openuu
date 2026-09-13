use super::*;

// Keyed by display index: the cursor lives on whichever CRTC the pointer is over and every other
// stream reports a hidden sentinel, which under a single global would clobber it.
#[derive(Clone)]
pub struct DrmCursorData {
    pub id: u64,
    pub width: i32,
    pub height: i32,
    pub hotx: i32,
    pub hoty: i32,
    pub colors: Vec<u8>,
}

pub(super) static DRM_CURSOR: Mutex<BTreeMap<i32, (u64, DrmCursorData)>> = Mutex::new(BTreeMap::new());
// Monotonic per-stream tag: a rebuilt stream reuses the display index, so a torn-down stream drops
// its entry ONLY if the epoch still matches.
pub(super) static DRM_CURSOR_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(super) fn next_cursor_epoch() -> u64 {
    DRM_CURSOR_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// Compare-and-set: a still-draining predecessor stream (older epoch) must not overwrite the entry a
// replacement stream (newer epoch) already published. Only accept a write whose epoch is at least
// the stored one.
pub(super) fn set_drm_cursor(display: i32, epoch: u64, c: DrmCursorData) {
    let mut map = DRM_CURSOR.lock().unwrap();
    match map.get(&display) {
        Some((stored, _)) if *stored > epoch => {}
        _ => {
            map.insert(display, (epoch, c));
        }
    }
}

pub(super) fn remove_drm_cursor(display: i32, epoch: u64) {
    let mut map = DRM_CURSOR.lock().unwrap();
    if map.get(&display).map(|(e, _)| *e) == Some(epoch) {
        map.remove(&display);
    }
}

/// Unrotate a wire cursor into the session orientation and publish it. The compositor
/// pre-rotates the bitmap it programs into the cursor plane, so over the unrotated video the
/// cursor alone would stay turned and its hotspot transposed (review finding 11 on
/// rustdesk#15889). The wire id hashes only the plane pixels and geometry, so a stream rebuilt
/// under a new transform resends the SAME id and the client's by-id cursor cache would keep the
/// old orientation: fold the transform in (the producer's own FNV step) so id and orientation
/// can never disagree. The hidden sentinel must survive untouched.
#[allow(clippy::too_many_arguments)]
pub(super) fn deliver_drm_cursor(
    display: i32,
    cursor_epoch: u64,
    id: u64,
    width: u32,
    height: u32,
    hotx: i32,
    hoty: i32,
    raw: Vec<u8>,
    t: i32,
) {
    let (width, height, hotx, hoty, colors) = if t == 90 || t == 270 {
        let mut turned = Vec::new();
        unrotate_bgra(&raw, width as usize, height as usize, t, &mut turned);
        let (hx, hy) = unrotate_hotspot(t, width as i32, height as i32, hotx, hoty);
        (height as i32, width as i32, hx, hy, turned)
    } else {
        (width as i32, height as i32, hotx, hoty, raw)
    };
    let id = fold_cursor_id(id, t);
    set_drm_cursor(
        display,
        cursor_epoch,
        DrmCursorData {
            id,
            width,
            height,
            hotx,
            hoty,
            colors,
        },
    );
}

pub(super) fn fold_cursor_id(id: u64, t: i32) -> u64 {
    if id == scrap::drm_reader::HIDDEN_CURSOR_ID {
        id
    } else {
        (id ^ t as u32 as u64).wrapping_mul(1099511628211)
    }
}

pub(super) fn with_drm_cursor<T>(f: impl Fn(&DrmCursorData) -> T) -> Option<T> {
    let map = DRM_CURSOR.lock().unwrap();
    map.values()
        .map(|(_, c)| c)
        .find(|c| c.id != scrap::drm_reader::HIDDEN_CURSOR_ID)
        .or_else(|| map.values().map(|(_, c)| c).next())
        .map(f)
}

pub fn drm_cursor_id() -> Option<u64> {
    with_drm_cursor(|c| c.id)
}

/// Snapshot of the DRM hardware cursor, or None. The pixels are premultiplied ARGB and are passed
/// through as-is, like the XFixes path, so the client sees one cursor format from either backend.
pub fn drm_cursor() -> Option<DrmCursorData> {
    with_drm_cursor(|c| c.clone())
}
