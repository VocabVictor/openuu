pub(super) fn percentile(values: &[f64], p: f64) -> f64 {
    assert!(!values.is_empty());
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

// Count a fall and subsequent rise of at least `amplitude`, ignoring smaller
// reversals. A one-way reduction or recovery is not an oscillation.
pub(super) fn round_trips(values: &[f64], amplitude: f64) -> usize {
    let mut peak = values[0];
    let mut trough = peak;
    let mut falling = false;
    let mut count = 0;
    for &value in &values[1..] {
        if falling {
            trough = trough.min(value);
            if value - trough >= amplitude {
                count += 1;
                peak = value;
                falling = false;
            }
        } else {
            peak = peak.max(value);
            if peak - value >= amplitude {
                trough = value;
                falling = true;
            }
        }
    }
    count
}

#[test]
pub(super) fn oscillation_metric_distinguishes_recovery_and_small_jitter() {
    assert_eq!(round_trips(&[30.0, 29.0, 30.0, 28.0, 30.0], 7.5), 0);
    assert_eq!(round_trips(&[30.0, 20.0, 10.0], 7.5), 0);
    assert_eq!(round_trips(&[10.0, 20.0, 30.0], 7.5), 0);
    assert_eq!(round_trips(&[30.0, 10.0, 30.0, 20.0, 30.0], 7.5), 2);
    assert_eq!(round_trips(&[0.49, 0.17, 0.49], 0.67 * 0.25), 1);
}

pub(super) struct Oscillation {
    pub(super) mean_fps: f64,
    pub(super) mean_ratio: f64,
    pub(super) fps_span: f64,
    pub(super) ratio_span: f64,
    pub(super) fps_cycles_per_min: f64,
    pub(super) ratio_cycles_per_min: f64,
    pub(super) queue_p95_ms: f64,
}

pub(super) fn permanent_drop(seeds: std::ops::RangeInclusive<u64>) {
    use super::super::sim::{self, Summary};

    pub(super) const STEADY_START_MS: u32 = 120_000;
    pub(super) const END_MS: u32 = 600_000;
    let cases = [
        ("bandwidth_halved_30", 3000.0),
        ("bandwidth_halved_fixed_rate_30", 2000.0),
        ("bandwidth_halved_fixed_rate_no_abr_30", 3000.0),
    ];
    println!("Permanent drop: 8 -> 2.5 Mbps at 60 s; duration 600 s; seeds {seeds:?}.");
    println!("Oscillation/queue window: 120-600 s. Spans are p95-p5; round trips require 25% of the requested FPS/ratio in each direction. Delivered FPS/frame age cover 15-600 s.");
    println!("| scenario | capacity wobble | steady mean FPS (median) | mean ratio (median) | FPS span (p90) | ratio span (p90) | FPS cycles/min (p90) | ratio cycles/min (p90) | queue p95 (p90) | delivered FPS (median) | frame age p95 (p90) |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut cases_run = 0;
    for mut sc in sim::scenarios() {
        let Some((_, queue_bound_ms)) = cases.iter().find(|(name, _)| *name == sc.name) else {
            continue;
        };
        cases_run += 1;
        sc.seconds = END_MS / 1000;
        sc.link.capacity_kbps = vec![(0, 8000.0), (60_000, 2500.0)];
        let original_wobble = sc.link.wobble;
        for wobble in [0.0, original_wobble] {
            sc.link.wobble = wobble;
            let mut reports = Vec::new();
            let mut oscillations = Vec::new();
            for seed in seeds.clone() {
                sc.seed = seed;
                let report = sim::run(&sc);
                assert!(
                    report
                        .trace
                        .iter()
                        .filter(|(t, ..)| (30_000..60_000).contains(t))
                        .all(|(_, fps, _, _)| *fps == sc.limit),
                    "healthy pre-drop phase: {} seed {seed}",
                    sc.name
                );
                let steady: Vec<_> = report
                    .trace
                    .iter()
                    .filter(|(t, ..)| *t >= STEADY_START_MS)
                    .collect();
                let fps: Vec<_> = steady.iter().map(|(_, fps, _, _)| *fps as f64).collect();
                let ratios: Vec<_> = steady
                    .iter()
                    .map(|(_, _, _, ratio)| *ratio as f64)
                    .collect();
                let queues: Vec<_> = steady
                    .iter()
                    .map(|(_, _, queue, _)| *queue as f64)
                    .collect();
                let minutes = (END_MS - STEADY_START_MS) as f64 / 60_000.0;
                oscillations.push(Oscillation {
                    mean_fps: fps.iter().sum::<f64>() / fps.len() as f64,
                    mean_ratio: ratios.iter().sum::<f64>() / ratios.len() as f64,
                    fps_span: percentile(&fps, 0.95) - percentile(&fps, 0.05),
                    ratio_span: percentile(&ratios, 0.95) - percentile(&ratios, 0.05),
                    fps_cycles_per_min: round_trips(&fps, sc.limit as f64 * 0.25) as f64 / minutes,
                    ratio_cycles_per_min: round_trips(&ratios, sc.quality.ratio() as f64 * 0.25)
                        as f64
                        / minutes,
                    queue_p95_ms: percentile(&queues, 0.95),
                });
                reports.push(report);
            }
            let metric = |field: fn(&Oscillation) -> f64, p| {
                percentile(&oscillations.iter().map(field).collect::<Vec<_>>(), p)
            };
            let summary = Summary::of(&reports);
            let queue_p95 = metric(|o| o.queue_p95_ms, 0.9);
            println!("| {} | {:.0}% | {:.1} | {:.3} | {:.1} | {:.3} | {:.2} | {:.2} | {:.0} ms | {:.1} | {} ms |",
                sc.name, wobble * 100.0, metric(|o| o.mean_fps, 0.5),
                metric(|o| o.mean_ratio, 0.5),
                metric(|o| o.fps_span, 0.9), metric(|o| o.ratio_span, 0.9),
                metric(|o| o.fps_cycles_per_min, 0.9), metric(|o| o.ratio_cycles_per_min, 0.9),
                queue_p95, summary.delivered_median, summary.frame_age_p95_p90);
            // Queue and frame-age limits reuse the transient-drop budgets;
            // oscillation statistics are diagnostic.
            assert!(
                queue_p95 < *queue_bound_ms,
                "{}: steady queue {queue_p95} ms",
                sc.name
            );
            assert!(
                (summary.frame_age_p95_p90 as f64) < *queue_bound_ms,
                "{summary:?}"
            );
            assert!(
                summary.delivered_median >= sc.limit as f64 * 0.4,
                "{summary:?}"
            );
        }
    }
    assert_eq!(cases_run, cases.len());
}

#[test]
pub(super) fn permanent_capacity_drop_600s() {
    permanent_drop(super::super::sim::SEEDS);
}

#[test]
#[ignore = "extended permanent-drop coverage over 100 held-out seeds"]
pub(super) fn permanent_capacity_drop_held_out_seeds() {
    permanent_drop(21..=120);
}
