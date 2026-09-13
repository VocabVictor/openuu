use super::*;

pub(in crate::server) fn get_capturer_for_display(
    display_idx: usize,
) -> ResultType<super::super::video_service::CapturerInfo> {
    if is_x11() {
        bail!("Do not call this function if not wayland");
    }
    // DRM/KMS capture path: build the capturer straight from the service `_drm` stream, bypassing
    // the PipeWire CAP_DISPLAY_INFO machinery entirely. `is_available()` is a GLOBAL verdict, so a
    // per-display DRM failure (an ungrabbable/demoted CRTC, or — after the phase-2 split — a
    // render-node-absent seat or a convert failure on the unprivileged side) must NOT propagate out
    // and restart-loop this per-display video service. Instead fall THROUGH to PipeWire for just this
    // display; the other DRM outputs keep streaming over DRM.
    // The ONE gate that keeps the probing form on purpose: this runs on the plain video thread,
    // not an async executor, and it is the capture-build path, so a definitive verdict is worth
    // seconds here. It is also what makes a cold cache recoverable at all -- warm_availability
    // gives up after its attempts, so if EVERY gate were cache-only a --server that started
    // before the root service would never see DRM again for the rest of its life.
    #[cfg(feature = "drm")]
    if super::super::drm_capturer::is_available() {
        match super::super::drm_capturer::get_capturer_info(display_idx) {
            Ok(info) => return Ok(info),
            Err(e) => {
                log::warn!(
                    "drm capturer for display {} unavailable ({:#}); falling back to PipeWire",
                    display_idx,
                    e
                );
                ensure_pipewire_inited()?;
            }
        }
    }
    // Resolved BEFORE the read guard below, deliberately. `get_display_infos` runs
    // `augment_with_wayland_geometry`, which is a compositor output roundtrip, and `clear()` takes
    // the WRITE guard on every capturer teardown -- which is exactly what is happening when a DRM
    // display is demoted or flapping, i.e. precisely when this path runs. Holding the read guard
    // across that roundtrip would stall every concurrent teardown for its duration, and the value
    // does not depend on anything inside the guard.
    #[cfg(feature = "drm")]
    let drm_advertised = if super::super::drm_capturer::is_available_cached() {
        match super::super::drm_capturer::get_display_infos() {
            Some(list) => Some((list.get(display_idx).cloned(), list.len() == 1)),
            None => Some((None, false)),
        }
    } else {
        None
    };
    let cap_map = CAP_DISPLAY_INFO.read().unwrap();
    // Serve ONLY the exact PipeWire entry for this index. Do NOT fall back to another index's
    // `CapDisplayInfo`: `CapturerPtr` is a bare `*mut Capturer` cloned by raw-pointer copy, so aliasing
    // one entry to two `display_idx` values would let two video-service threads call `frame()` on the
    // same `Recorder` with no lock (data race / UB), and it would also mis-map input against the wrong
    // rect. DRM and PipeWire do not share an index space (the portal often exposes one whole-desktop
    // stream at index 0), so a demoted non-primary DRM index has no PipeWire entry here; that case is
    // handled at the source by dropping the demoted display from the advertised list (see
    // drm_capturer demotion) so the client re-enumerates against a consistent list, rather than being
    // papered over with a shared/mismatched capturer.
    if let Some(addr) = cap_map.get(&display_idx) {
        let cap_display_info: *const CapDisplayInfo = *addr as _;
        unsafe {
            let cap_display_info = &*cap_display_info;
            let rect = cap_display_info.rects[cap_display_info.current];
            // Reaching here with DRM active means get_capturer_info bailed (a demoted display) and
            // we fell through to PipeWire. Serve this stream ONLY if its rect matches the
            // geometry we advertised for this index. The portal typically exposes one whole-desktop
            // stream, so on a multi-monitor host that rect is the FULL desktop while the advertised DRM
            // geometry is a single connector -> serving it would stretch the frame and offset all
            // input. Bail instead; get_display_infos advertised the display offline, so the client
            // re-enumerates against a consistent list. A single-display host matches (whole-desktop ==
            // that display) and is served normally. On a pure-PipeWire host is_available() is false and
            // this guard is skipped, preserving upstream behavior exactly.
            #[cfg(feature = "drm")]
            if let Some((advertised, single_display)) = drm_advertised {
                if let Some(advertised) = advertised {
                    // BOTH SIDES ARE PHYSICAL, so compare them raw. Traced rather than assumed,
                    // because it was twice "corrected" to a scale conversion that broke it:
                    // `rect` is built above from `Display::width()/height()`, and the WAYLAND
                    // variant of those returns `physical_width()/physical_height()`
                    // (scrap `common/wayland.rs`), i.e. `PipeWireCapturable.physical_size`.
                    // `try_fix_logical_size` only repairs the capturable's SEPARATE
                    // `logical_size` field and never touches `physical_size`, so the rect is not
                    // logical. The advertised DRM geometry is physical too, in DELIVERED
                    // orientation: `augment_with_wayland_geometry` transposes width/height for a
                    // 90/270 output (rustdesk#15886). Whether the portal's caps arrive rotated
                    // is UNMEASURED on a rotated display (pipewiresrc does not apply
                    // SPA_META_VideoTransform), so the size half accepts either orientation
                    // rather than gambling a permanent offline on one of them. Dividing a side
                    // by the scale would still be wrong: logical against physical.
                    //
                    // The size check is what tells one connector apart from the whole-desktop
                    // rect the portal usually exposes. It is skipped only when BOTH sides say
                    // there is a single display -- the DRM list has one entry and the PipeWire
                    // map has one -- because only then is "the whole-desktop stream IS this
                    // display" true by construction. (The portal can report a different physical
                    // size for a Full Workspace selection than the connector's mode, which is why
                    // that case needs the carve-out at all.) The DRM count alone is not enough:
                    // a monitor on a card the service cannot open is missing from the DRM list
                    // while the compositor still drives it.
                    let single_display = single_display && cap_display_info.num == 1;
                    // Exact orientation only: a transposed stream would be encoded at the
                    // PipeWire dimensions while the client keeps the advertised (rotated) ones,
                    // and no wayland path ever reconciles the two, so every frame would be
                    // rejected client-side. Falling into the bail instead advertises the display
                    // offline, which the client recovers from by re-enumerating.
                    let size_matches = advertised.width as usize == rect.1
                        && advertised.height as usize == rect.2;
                    let transposed = advertised.width as usize == rect.2
                        && advertised.height as usize == rect.1;
                    // The single-display carve-out forgives a size DIFFERENCE (a Full Workspace
                    // stream may report the workspace, not the mode), but never a transposed
                    // pair: that is the same served-vs-advertised orientation split as above,
                    // and it blanks the client the same way.
                    let consistent = advertised.x == rect.0 .0
                        && advertised.y == rect.0 .1
                        && (size_matches || (single_display && !transposed));
                    if !consistent {
                        // Recorded so the lone-display carve-out in `mark_demoted_displays` makes
                        // the "advertised offline" below true for a single display too, instead of
                        // restart-looping against a stream nothing can serve.
                        super::super::drm_capturer::mark_fallback_rejected(display_idx);
                        bail!(
                            "drm display {} demoted with no geometry-consistent PipeWire stream{} (advertised {}x{}+{}+{} vs stream {}x{}+{}+{}); advertised offline",
                            display_idx,
                            if transposed {
                                " - stream is transposed vs advertised"
                            } else {
                                ""
                            },
                            advertised.width,
                            advertised.height,
                            advertised.x,
                            advertised.y,
                            rect.1,
                            rect.2,
                            rect.0 .0,
                            rect.0 .1
                        );
                    }
                }
            }
            Ok(super::super::video_service::CapturerInfo {
                origin: rect.0,
                width: rect.1,
                height: rect.2,
                ndisplay: cap_display_info.num,
                current: cap_display_info.current,
                privacy_mode_id: 0,
                _capturer_privacy_mode_id: 0,
                capturer: Box::new(cap_display_info.capturer.clone()),
            })
        }
    } else {
        bail!(
            "Failed to get capturer display info for display {}",
            display_idx
        );
    }
}

pub fn common_get_error() -> String {
    if DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() {
        if DISTRO.version_id < "21".to_owned() {
            return "".to_owned();
        }
    } else {
        // to-do: check other distros
    }
    return "".to_owned();
}
