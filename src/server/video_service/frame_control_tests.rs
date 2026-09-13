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
