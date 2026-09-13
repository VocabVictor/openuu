//! A link too thin for the preset: the backlog the first seconds build must drain, and
//! the bitrate must come back to what the link can carry once it has.
use super::super::sim;

/// Queue delay under which the backlog counts as drained.
const DRAINED_MS: u32 = 1_000;
/// Seeds enough to show the behaviour is not one link's luck.
const SEEDS: std::ops::RangeInclusive<u64> = 1..=5;

struct Drain {
    seed: u64,
    peak_queue_ms: u32,
    drained_at_ms: Option<u32>,
    /// Ratio once the backlog cleared, as a fraction of the ratio before it did.
    ratio_after: f32,
    steady_queue_p95_ms: u32,
    final_fps: u32,
}

fn measure(seed: u64) -> Drain {
    let mut sc = sim::scenarios()
        .into_iter()
        .find(|s| s.name == "relay_0_3x_30")
        .expect("the thin relay scenario");
    sc.seed = seed;
    let report = sim::run(&sc);
    let peak_queue_ms = report.trace.iter().map(|(_, _, q, _)| *q).max().unwrap_or(0);
    let drained_at_ms = report
        .trace
        .iter()
        .find(|(t, _, q, _)| *t > 5_000 && *q < DRAINED_MS)
        .map(|(t, ..)| *t);
    let ratio_min = report
        .trace
        .iter()
        .map(|(_, _, _, r)| *r)
        .fold(f32::MAX, f32::min);
    let ratio_after = report.final_ratio / ratio_min.max(f32::EPSILON);
    let mut steady: Vec<u32> = report
        .trace
        .iter()
        .filter(|(t, ..)| *t >= 60_000)
        .map(|(_, _, q, _)| *q)
        .collect();
    steady.sort_unstable();
    let steady_queue_p95_ms = steady
        .get(steady.len().saturating_mul(95) / 100)
        .copied()
        .unwrap_or(u32::MAX);
    Drain {
        seed,
        peak_queue_ms,
        drained_at_ms,
        ratio_after,
        steady_queue_p95_ms,
        final_fps: report.final_fps,
    }
}

#[test]
fn a_thin_link_drains_its_startup_backlog_and_then_uses_what_it_has() {
    println!("| seed | peak queue ms | drained at ms | steady queue p95 ms | ratio after / floor | final fps |");
    println!("|---:|---:|---:|---:|---:|---:|");
    let mut unmet = Vec::new();
    for seed in SEEDS {
        let d = measure(seed);
        println!(
            "| {} | {} | {:?} | {} | {:.2} | {} |",
            d.seed, d.peak_queue_ms, d.drained_at_ms, d.steady_queue_p95_ms, d.ratio_after, d.final_fps
        );
        // The backlog a cold start builds on a link at a third of the preset must be
        // gone within about half a minute, not left to trickle away for the rest of the
        // session.  The floor bounds how fast it can go: 240 kbps under a link carrying
        // 1.2 Mbps drains a second of queue every 1.2 seconds.
        if !d.drained_at_ms.is_some_and(|ms| ms <= 35_000) {
            unmet.push(format!("seed {}: drained at {:?}", d.seed, d.drained_at_ms));
        }
        // Once drained it must stay drained: the controller settled on what the link carries.
        if d.steady_queue_p95_ms >= DRAINED_MS {
            unmet.push(format!("seed {}: steady queue {} ms", d.seed, d.steady_queue_p95_ms));
        }
        // The queue a thin link builds is bounded by how fast the controller reads it.
        if d.peak_queue_ms > 25_000 {
            unmet.push(format!("seed {}: peak queue {} ms", d.seed, d.peak_queue_ms));
        }
        // Draining is temporary. The bitrate comes back up once the queue is clear.
        if d.ratio_after < 1.2 {
            unmet.push(format!("seed {}: ratio stayed at {:.2} of its low", d.seed, d.ratio_after));
        }
    }
    assert!(unmet.is_empty(), "{unmet:?}");
}
