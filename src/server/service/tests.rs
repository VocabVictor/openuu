//! What a service loop does when nothing is connected to it.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Default)]
struct Ticks;

impl Reset for Ticks {
    fn reset(&mut self) {}
}

fn service(name: &str) -> EmptyExtraFieldService {
    EmptyExtraFieldService::new(name.to_owned(), false)
}

/// A `repeat` service whose callback counts the rounds it was actually called for.
fn counting_repeat(name: &str, interval_ms: u64) -> (EmptyExtraFieldService, Arc<AtomicUsize>) {
    let svc = service(name);
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    GenericService::repeat::<Ticks, _, _>(&svc.clone(), interval_ms, move |_, _| {
        counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    (svc, calls)
}

fn wait_for(deadline_ms: u64, mut done: impl FnMut() -> bool) -> time::Duration {
    let start = time::Instant::now();
    while !done() && start.elapsed() < time::Duration::from_millis(deadline_ms) {
        thread::sleep(time::Duration::from_millis(2));
    }
    start.elapsed()
}

#[test]
fn an_idle_service_sleeps_instead_of_polling() {
    let (svc, calls) = counting_repeat("idle-repeat", 33);
    thread::sleep(time::Duration::from_millis(500));
    let wakeups = svc.sp.wakeups();
    // Polling every 33 ms would be fifteen rounds; the hibernating loop wakes on its
    // backstop, which is a second.
    assert!(wakeups <= 2, "{wakeups} rounds while idle");
    assert_eq!(calls.load(Ordering::Relaxed), 0, "nothing to do");
    svc.sp.join();
}

#[test]
fn a_subscriber_wakes_an_idle_service_at_once() {
    let (svc, calls) = counting_repeat("wake-repeat", 33);
    thread::sleep(time::Duration::from_millis(200));
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let waited = wait_for(500, || {
        if calls.load(Ordering::Relaxed) == 0 && !svc.sp.has_subscribes() {
            svc.sp.on_subscribe(ConnInner::default());
        }
        calls.load(Ordering::Relaxed) > 0
    });
    assert!(calls.load(Ordering::Relaxed) > 0, "the loop never woke");
    assert!(
        waited < time::Duration::from_millis(100),
        "woke after {waited:?}"
    );
    svc.sp.join();
}

#[test]
fn a_subscribed_service_keeps_its_cadence() {
    let (svc, calls) = counting_repeat("cadence-repeat", 30);
    svc.sp.on_subscribe(ConnInner::default());
    thread::sleep(time::Duration::from_millis(300));
    let rounds = calls.load(Ordering::Relaxed);
    assert!((5..=15).contains(&rounds), "{rounds} rounds in 300 ms");
    svc.sp.join();
}

#[test]
fn losing_the_last_subscriber_puts_the_service_back_to_sleep() {
    let (svc, calls) = counting_repeat("unsub-repeat", 30);
    svc.sp.on_subscribe(ConnInner::default());
    wait_for(300, || calls.load(Ordering::Relaxed) > 0);
    svc.sp.on_unsubscribe(ConnInner::default().id());

    let before = svc.sp.wakeups();
    thread::sleep(time::Duration::from_millis(400));
    let idle_rounds = svc.sp.wakeups() - before;
    assert!(idle_rounds <= 2, "{idle_rounds} rounds after unsubscribing");
    svc.sp.join();
}

/// Stopping a sleeping service must not wait out its backstop.
#[test]
fn joining_a_sleeping_service_returns_promptly() {
    let (svc, _) = counting_repeat("join-repeat", 30);
    thread::sleep(time::Duration::from_millis(100));
    let start = time::Instant::now();
    svc.sp.join();
    assert!(
        start.elapsed() < time::Duration::from_millis(300),
        "join took {:?}",
        start.elapsed()
    );
}

#[test]
fn a_run_service_hibernates_too() {
    let svc = service("idle-run");
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    GenericService::run(&svc.clone(), move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    thread::sleep(time::Duration::from_millis(500));
    let wakeups = svc.sp.wakeups();
    assert!(wakeups <= 2, "{wakeups} rounds while idle");
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    svc.sp.on_subscribe(ConnInner::default());
    let waited = wait_for(500, || calls.load(Ordering::Relaxed) > 0);
    assert!(calls.load(Ordering::Relaxed) > 0, "the loop never woke");
    assert!(waited < time::Duration::from_millis(100), "woke after {waited:?}");
    svc.sp.join();
}
