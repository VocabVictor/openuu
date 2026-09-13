//! Reaping the child processes a session leaves behind.
use super::service::Wakeup;
use super::start::{reap, Reapable};
use std::{
    rc::Rc,
    cell::Cell,
    sync::Arc,
    time::{Duration, Instant},
};

/// A child that is over when it is told to be, counting how often it was asked.
struct Fake {
    over: bool,
    asked: Rc<Cell<u32>>,
}

impl Reapable for Fake {
    fn finished(&mut self) -> bool {
        self.asked.set(self.asked.get() + 1);
        self.over
    }
}

fn children(over: &[bool]) -> (Vec<Fake>, Rc<Cell<u32>>) {
    let asked = Rc::new(Cell::new(0));
    let list = over
        .iter()
        .map(|over| Fake {
            over: *over,
            asked: asked.clone(),
        })
        .collect();
    (list, asked)
}

#[test]
fn with_nothing_to_reap_the_thread_waits_to_be_woken() {
    let (mut list, asked) = children(&[]);
    assert_eq!(reap(&mut list), None, "no children, nothing to come back for");
    assert_eq!(asked.get(), 0);
}

#[test]
fn a_child_that_has_exited_is_dropped_and_ends_the_polling() {
    let (mut list, asked) = children(&[true]);
    assert_eq!(reap(&mut list), None);
    assert!(list.is_empty(), "the exited child was not dropped");
    assert_eq!(asked.get(), 1);
}

#[test]
fn a_child_still_running_is_kept_and_looked_at_again_soon() {
    let (mut list, _) = children(&[false]);
    let next = reap(&mut list).expect("a running child is polled");
    assert!(next <= Duration::from_millis(200), "{next:?}");
    assert_eq!(list.len(), 1);
}

#[test]
fn the_ones_that_are_over_go_and_the_rest_stay() {
    let (mut list, asked) = children(&[true, false, true, false]);
    assert!(reap(&mut list).is_some());
    assert_eq!(list.len(), 2, "both running children stayed");
    assert!(list.iter().all(|child| !child.over));
    assert_eq!(asked.get(), 4, "every child was asked exactly once");
}

/// What replaces the polling: the reaper sleeps until something is spawned. A wait that
/// is never notified costs its own timeout, which is why the loop has one.
#[test]
fn a_spawned_child_wakes_a_waiting_reaper() {
    let wakeup: Arc<Wakeup> = Default::default();
    let waiter = wakeup.clone();
    let woke = std::thread::spawn(move || {
        let start = Instant::now();
        waiter.wait_for_change(Duration::from_secs(30));
        start.elapsed()
    });
    std::thread::sleep(Duration::from_millis(50));
    wakeup.notify();
    let waited = woke.join().expect("the waiting thread");
    assert!(waited < Duration::from_secs(5), "woke after {waited:?}");
}

#[test]
fn a_reaper_nobody_wakes_sleeps_for_its_own_timeout() {
    let wakeup: Arc<Wakeup> = Default::default();
    let start = Instant::now();
    wakeup.wait_for_change(Duration::from_millis(120));
    assert!(start.elapsed() >= Duration::from_millis(100), "returned early");
}
