use super::*;
use std::collections::HashSet;
use hbb_common::tokio::{self, time::{interval, interval_at, sleep, Duration, Instant, Interval}};

    #[inline]
fn get_timestamp_secs() -> u128 {
        (std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .as_millis()
            + 500)
            / 1000
    }

    fn interval_maker() -> Interval {
        interval(Duration::from_secs(1))
    }

    fn interval_at_maker() -> Interval {
        interval_at(
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
        )
    }

    // The deadline must hold against a peer that keeps the receive side ready. `select!` rebuilds
    // its arms every iteration, so a relative sleep would be restarted by every datagram and the
    // punch would run for as long as the peer keeps talking, with no outer timeout to stop it.

    #[tokio::test]
    async fn test_RustDesk_interval() {
        let base_intervals = [interval_maker, interval_at_maker];
        for maker in base_intervals.into_iter() {
            let mut tokio_timer = maker();
            let mut tokio_times = Vec::new();
            let mut timer = rustdesk_interval(maker());
            let mut times = Vec::new();
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        if tokio_times.len() >= 10 && times.len() >= 10 {
                            break;
                        }
                        times.push(get_timestamp_secs());
                    }
                    _ = tokio_timer.tick() => {
                        if tokio_times.len() >= 10 && times.len() >= 10 {
                            break;
                        }
                        tokio_times.push(get_timestamp_secs());
                    }
                }
            }
            assert_eq!(times, tokio_times);
        }
    }

    #[tokio::test]
    async fn test_tokio_time_interval_sleep() {
        let mut timer = interval_maker();
        let mut times = Vec::new();
        sleep(Duration::from_secs(3)).await;
        loop {
            tokio::select! {
                _ = timer.tick() => {
                    times.push(get_timestamp_secs());
                    if times.len() == 5 {
                        break;
                    }
                }
            }
        }
        let times2: HashSet<u128> = HashSet::from_iter(times.clone());
        assert_eq!(times.len(), times2.len() + 3);
    }

    // ThrottledInterval tick less times than tokio interval, if there're sleeps
    #[allow(non_snake_case)]
    #[tokio::test]
    async fn test_RustDesk_interval_sleep() {
        let base_intervals = [interval_maker, interval_at_maker];
        for (i, maker) in base_intervals.into_iter().enumerate() {
            let mut timer = rustdesk_interval(maker());
            let mut times = Vec::new();
            sleep(Duration::from_secs(3)).await;
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        times.push(get_timestamp_secs());
                        if times.len() == 5 {
                            break;
                        }
                    }
                }
            }
            // No multiple ticks in the `interval` time.
            // Values in "times" are unique and are less than normal tokio interval.
            // See previous test (test_tokio_time_interval_sleep) for comparison.
            let times2: HashSet<u128> = HashSet::from_iter(times.clone());
            assert_eq!(times.len(), times2.len(), "test: {}", i);
        }
    }

    #[test]
    fn test_duration_multiplication() {
        let dur = Duration::from_secs(1);

        assert_eq!(dur * 2, Duration::from_secs(2));
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.9),
            Duration::from_millis(900)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923),
            Duration::from_millis(923)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923 * 1e-3),
            Duration::from_micros(923)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923 * 1e-6),
            Duration::from_nanos(923)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923 * 1e-9),
            Duration::from_nanos(1)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.5 * 1e-9),
            Duration::from_nanos(1)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.499 * 1e-9),
            Duration::from_nanos(0)
        );
    }
