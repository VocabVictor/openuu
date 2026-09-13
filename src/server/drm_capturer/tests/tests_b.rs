use super::*;

pub(super) fn zero_frame_streak_of(c: &IpcDrmCapturer) -> u32 {
    let key = c.connector.clone().expect("this check needs an identity");
    DRM_DISPLAY_HEALTH
        .lock()
        .unwrap()
        .get(&key)
        .map(|h| h.zero_frame_streak)
        .unwrap_or(0)
}

pub(super) fn put_frame(c: &IpcDrmCapturer, w: usize, h: usize) {
    let mut buf = c.shared.slot.lock().unwrap().take_free().unwrap_or_default();
    buf.clear();
    buf.resize(w * h * 4, 0);
    let mut slot = c.shared.slot.lock().unwrap();
    slot.publish(w, h, Pixfmt::BGRA, buf);
}

#[test]
pub(super) fn a_delivered_frame_clears_the_streak_but_keeps_the_cadence_and_the_convert_verdict() {
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
pub(super) fn frame_of_the_session_size_is_delivered() {
    let mut c = capturer_with(Some((64, 32)));
    put_frame(&c, 64, 32);
    assert!(
        matches!(c.frame(Duration::from_millis(50)), Ok(_)),
        "a frame matching the session geometry must be delivered"
    );
    assert!(c.got_frame);
}

#[test]
pub(super) fn a_smaller_frame_ends_the_session_instead_of_being_encoded() {
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
pub(super) fn a_first_frame_that_never_matched_counts_as_a_session_without_frames() {
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
pub(super) fn a_larger_frame_ends_the_session_too() {
    let mut c = capturer_with(Some((1280, 720)));
    put_frame(&c, 1920, 1080);
    assert!(matches!(c.frame(Duration::from_millis(50)), Err(_)));
}

#[test]
pub(super) fn unknown_session_size_delivers_whatever_arrives() {
    let mut c = capturer_with(None);
    put_frame(&c, 800, 600);
    assert!(matches!(c.frame(Duration::from_millis(50)), Ok(_)));
}

pub(super) fn drm_display(name: &str, w: u32, h: u32) -> DrmDisplayInfo {
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

pub(super) fn wl_display(
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
