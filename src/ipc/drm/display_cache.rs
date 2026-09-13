use super::*;

pub(super) static DRM_DISPLAY_CACHE: std::sync::Mutex<Vec<DrmDisplayInfo>> = std::sync::Mutex::new(Vec::new());

/// Bumped only when a change altered `DRM_DISPLAY_CACHE`; Release orders it after the cache write.
pub(super) static DRM_DISPLAY_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Displays this reader serves, plus the identity (`device:connector`) of each undriven output.
pub(super) fn drm_displays_from_reader(
    reader: &mut scrap::drm_reader::DrmReader,
    device: &str,
) -> (Vec<DrmDisplayInfo>, Vec<String>) {
    let render_node = reader.render_node().unwrap_or_default();
    let mut undriven = Vec::new();
    let displays: Vec<DrmDisplayInfo> = reader
        .displays()
        .into_iter()
        // Only outputs bound to a CRTC: a CONNECTED-but-unbound connector enumerates with
        // `crtc_id == 0`, and `open(crtc=0)` auto-selects the FIRST ACTIVE CRTC and streams ITS frames.
        .filter(|d| {
            if !d.active || d.crtc_id == 0 {
                undriven.push(format!("{device}:{name}", name = d.name));
                return false;
            }
            true
        })
        .map(|d| DrmDisplayInfo {
            name: d.name,
            crtc_id: d.crtc_id,
            x: d.x,
            y: d.y,
            width: d.width,
            height: d.height,
            active: d.active,
            render_node: render_node.clone(),
            device: device.to_owned(),
        })
        .collect();
    (displays, undriven)
}

/// Active displays of every DRM device + the connected-but-undriven identities, from ONE look.
pub(super) fn drm_enumerate_all_displays() -> (Vec<DrmDisplayInfo>, Vec<String>) {
    if let Some(devices) = scrap::drm_reader::list_devices() {
        if devices.len() > 1 {
            log::info!(
                "drm: {} DRM devices: {}",
                devices.len(),
                devices
                    .iter()
                    .map(|d| format!(
                        "{} ({}, render {})",
                        d.path,
                        d.display_count,
                        if d.render_node.is_empty() { "none" } else { &d.render_node }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        let mut all = Vec::new();
        let mut undriven_total = Vec::new();
        let mut any_opened = false;
        for dev in devices {
            if let Some(mut r) = scrap::drm_reader::DrmReader::open(Some(&dev.path), 0) {
                any_opened = true;
                let (mut got, mut undriven) = drm_displays_from_reader(&mut r, &dev.path);
                all.append(&mut got);
                undriven_total.append(&mut undriven);
            } else if dev.display_count == 0 {
                log::debug!(
                    "drm: {} has no active display and did not open; cannot tell whether it has a \
                     connected output that is merely switched off",
                    dev.path
                );
            }
        }
        // Take this even when the list is EMPTY: the fallback re-keys identities under `device = ""`.
        if any_opened {
            return (all, undriven_total);
        }
    }
    // Auto-detect alone is not enough: it picks a card that is SCANNING OUT. Measured on the T2 with
    // the panel idle-disabled it binds card0 (the Touch Bar); the panel on card2 is invisible to it.
    let mut all = Vec::new();
    let mut undriven_total = Vec::new();
    let mut paths: Vec<std::path::PathBuf> = match std::fs::read_dir("/dev/dri") {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit()))
            })
            .collect(),
        Err(err) => {
            log::debug!("drm: cannot read /dev/dri to enumerate cards: {err}");
            Vec::new()
        }
    };
    // Deterministic order, so the display list does not depend on directory order.
    paths.sort();
    let n_paths = paths.len();
    for p in paths {
        let Some(path) = p.to_str() else { continue };
        if let Some(mut r) = scrap::drm_reader::DrmReader::open(Some(path), 0) {
            let (mut got, mut undriven) = drm_displays_from_reader(&mut r, path);
            all.append(&mut got);
            undriven_total.append(&mut undriven);
        }
    }
    log::info!(
        "drm: enumerated /dev/dri directly ({} card path(s)): {} active display(s), {} connected \
         but undriven",
        n_paths,
        all.len(),
        undriven_total.len()
    );
    if all.is_empty() && undriven_total.is_empty() {
        if let Some(mut r) = scrap::drm_reader::DrmReader::open(None, 0) {
            log::info!("drm: no card enumerated by path; falling back to the auto-detected reader");
            return drm_displays_from_reader(&mut r, "");
        }
    }
    (all, undriven_total)
}
