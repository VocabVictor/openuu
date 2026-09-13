//! The capture loop's wait for the previous frame to be picked up by every connection.
use super::*;

fn notifier_for(display_idx: usize) {
    FRAME_FETCHED_NOTIFIERS
        .lock()
        .unwrap()
        .entry(display_idx)
        .or_insert_with(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            (tx, Arc::new(Mutex::new(rx)))
        });
}

#[test]
fn nothing_outstanding_returns_at_once() {
    let mut fc = VideoFrameController::new(9_001);
    let mut fetched = HashSet::new();
    let started = Instant::now();
    fc.try_wait_next(&mut fetched, 1000);
    assert!(fetched.is_empty());
    assert!(started.elapsed() < Duration::from_millis(200));
}

#[test]
fn the_window_ends_without_a_fetch_and_a_fetch_drains_the_rest() {
    let display = 9_002;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    fc.set_send(Instant::now(), HashSet::from([1, 2]));
    let mut fetched = HashSet::new();
    let started = Instant::now();
    fc.try_wait_next(&mut fetched, 100);
    assert!(fetched.is_empty());
    assert!(started.elapsed() >= Duration::from_millis(90));

    notify_video_frame_fetched(display, 1, Some(Instant::now()));
    notify_video_frame_fetched(display, 2, None);
    let started = Instant::now();
    fc.try_wait_next(&mut fetched, 1000);
    assert_eq!(fetched, HashSet::from([1, 2]));
    assert!(started.elapsed() < Duration::from_millis(500), "no wait once notified");
}

#[test]
fn a_fetch_arriving_during_the_wait_ends_it_early() {
    let display = 9_003;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    fc.set_send(Instant::now(), HashSet::from([7]));
    let notifier = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        notify_video_frame_fetched(display, 7, None);
    });
    let mut fetched = HashSet::new();
    let started = Instant::now();
    fc.try_wait_next(&mut fetched, 2000);
    notifier.join().unwrap();
    assert_eq!(fetched, HashSet::from([7]));
    assert!(started.elapsed() < Duration::from_millis(1000));
}

#[test]
fn a_connection_notified_by_id_reaches_the_display_it_watches() {
    let display = 9_004;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    fc.set_send(Instant::now(), HashSet::from([3]));
    notify_video_frame_fetched_by_conn_id(3, None);
    let mut fetched = HashSet::new();
    fc.try_wait_next(&mut fetched, 500);
    assert_eq!(fetched, HashSet::from([3]));
    DISPLAY_CONN_IDS.lock().unwrap().remove(&display);
}

#[test]
fn the_ack_window_follows_the_frame_period_or_the_round_trip_within_a_slice() {
    let spf = Duration::from_millis(33);
    assert_eq!(ack_wait_window(spf, None), Duration::from_millis(66));
    assert_eq!(ack_wait_window(spf, Some(10)), Duration::from_millis(66));
    assert_eq!(ack_wait_window(spf, Some(50)), Duration::from_millis(150));
    assert_eq!(ack_wait_window(spf, Some(400)), Duration::from_millis(300));
    assert_eq!(ack_wait_window(Duration::from_millis(10), None), Duration::from_millis(50));
}

#[test]
fn a_fetched_frame_lets_the_next_encode_go_at_once() {
    let display = 9_005;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    let mut hold = FetchHold::new();
    fc.set_send(Instant::now(), HashSet::from([1]));
    notify_video_frame_fetched(display, 1, None);
    hold.after_send(&mut fc, Duration::from_millis(300), || Ok(())).unwrap();
    let started = Instant::now();
    assert!(hold.may_encode(&mut fc, Duration::from_millis(300), || Ok(())).unwrap());
    assert!(started.elapsed() < Duration::from_millis(50));
    assert!(hold.last_wait_ms() < 100);
}

#[test]
fn an_unfetched_frame_holds_the_next_encode_until_it_is_fetched() {
    let display = 9_006;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    let mut hold = FetchHold::new();
    fc.set_send(Instant::now(), HashSet::from([1, 2]));
    notify_video_frame_fetched(display, 1, None);
    let window = Duration::from_millis(60);
    hold.after_send(&mut fc, window, || Ok(())).unwrap();
    assert!(!hold.may_encode(&mut fc, window, || Ok(())).unwrap(), "2 has not fetched");
    notify_video_frame_fetched(display, 2, None);
    assert!(hold.may_encode(&mut fc, window, || Ok(())).unwrap());
    assert!(hold.last_wait_ms() >= 100, "{}", hold.last_wait_ms());
}

#[test]
fn a_viewer_that_never_fetches_is_given_up_on_after_the_limit() {
    let display = 9_007;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    let mut hold = FetchHold::with_limit(Duration::from_millis(150));
    fc.set_send(Instant::now(), HashSet::from([1]));
    let window = Duration::from_millis(60);
    hold.after_send(&mut fc, window, || Ok(())).unwrap();
    let mut rounds = 0;
    while !hold.may_encode(&mut fc, window, || Ok(())).unwrap() {
        rounds += 1;
        assert!(rounds < 10, "the hold limit must end the hold");
    }
    assert!(hold.last_wait_ms() >= 150);
}

#[test]
fn the_tick_runs_between_slices_and_its_error_ends_the_wait() {
    let display = 9_008;
    notifier_for(display);
    let mut fc = VideoFrameController::new(display);
    let mut hold = FetchHold::new();
    fc.set_send(Instant::now(), HashSet::from([1]));
    let mut ticks = 0;
    let err = hold.after_send(&mut fc, Duration::from_millis(300), || {
        ticks += 1;
        hbb_common::bail!("privacy mode changed")
    });
    assert!(err.is_err());
    assert_eq!(ticks, 1);
}

#[test]
fn a_screen_that_is_being_used_is_waited_for_at_the_frame_rate() {
    let spf = Duration::from_millis(33);
    for still in 0..=STILL_AFTER {
        assert_eq!(capture_timeout(spf, still), spf, "{still} empty captures");
    }
}

#[test]
fn a_still_screen_is_waited_for_longer() {
    let spf = Duration::from_millis(33);
    let waited = capture_timeout(spf, STILL_AFTER + 1);
    assert!(waited > spf, "{waited:?}");
    assert_eq!(waited, capture_timeout(spf, 10_000), "and no longer than that");
}

/// The wait never goes below the frame period, so a loop that is already slow is not
/// made to spin faster than it would have.
#[test]
fn a_slow_frame_rate_keeps_its_own_period() {
    let spf = Duration::from_millis(500);
    assert_eq!(capture_timeout(spf, 0), spf);
    assert_eq!(capture_timeout(spf, 100), spf);
}
