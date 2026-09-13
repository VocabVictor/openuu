use super::*;

pub(super) fn capturer_with(session: Option<(usize, usize)>) -> IpcDrmCapturer {
    capturer_named(session, None)
}

// DRM_DISPLAY_HEALTH is process-wide and tests run in parallel: pass each test its OWN key.
pub(super) fn capturer_named(session: Option<(usize, usize)>, key: Option<&str>) -> IpcDrmCapturer {
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
pub(super) fn px_frame(labels: &[&[u8]], pad_bytes: usize) -> (Vec<u8>, usize, usize) {
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

pub(super) fn labels_of(buf: &[u8], w: usize, h: usize) -> Vec<Vec<u8>> {
    (0..h)
        .map(|y| (0..w).map(|x| buf[(y * w + x) * 4]).collect())
        .collect()
}

#[test]
pub(super) fn a_lone_display_goes_offline_only_when_its_fallback_was_rejected() {
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
pub(super) fn the_cursor_id_names_the_orientation_too() {
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
pub(super) fn unrotate_hotspot_follows_the_pixel_mapping() {
    // 3 wide x 2 tall, hotspot at (2,0) (top-right): after the 90 turn (left column to top
    // row) that pixel sits at (1,2) in the 2x3 result; 270 sends it to (0,0).
    assert_eq!(unrotate_hotspot(90, 3, 2, 2, 0), (1, 2));
    assert_eq!(unrotate_hotspot(270, 3, 2, 2, 0), (0, 0));
    assert_eq!(unrotate_hotspot(180, 3, 2, 2, 0), (0, 1));
    assert_eq!(unrotate_hotspot(0, 3, 2, 2, 0), (2, 0));
}

#[test]
pub(super) fn a_stale_snapshot_generation_asks_for_a_rebuild_without_blaming_the_display() {
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
pub(super) fn unrotate_90_maps_the_left_column_to_the_top_row() {
    // The measured anchor from rustdesk#15886: mutter transform=1 carries the panel bar down
    // the scanout's LEFT edge, and upright means that edge becomes the TOP row.
    let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
    let mut dst = Vec::new();
    unrotate_bgra(&src, w, h, 90, &mut dst);
    // src left column top-to-bottom = [1, 4]; clockwise puts it on the top row as [4, 1].
    assert_eq!(labels_of(&dst, h, w), vec![vec![4, 1], vec![5, 2], vec![6, 3]]);
}

#[test]
pub(super) fn unrotate_270_is_the_inverse_of_90() {
    let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
    let mut once = Vec::new();
    unrotate_bgra(&src, w, h, 90, &mut once);
    let mut back = Vec::new();
    unrotate_bgra(&once, h, w, 270, &mut back);
    assert_eq!(back, src);
}

#[test]
pub(super) fn unrotate_180_reverses_both_axes() {
    let (src, w, h) = px_frame(&[&[1, 2, 3], &[4, 5, 6]], 0);
    let mut dst = Vec::new();
    unrotate_bgra(&src, w, h, 180, &mut dst);
    assert_eq!(labels_of(&dst, w, h), vec![vec![6, 5, 4], vec![3, 2, 1]]);
}

#[test]
pub(super) fn unrotate_reads_padded_strides_and_writes_tight() {
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
pub(super) fn a_rotated_session_delivers_rotated_frames_and_guards_in_rotated_dims() {
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
