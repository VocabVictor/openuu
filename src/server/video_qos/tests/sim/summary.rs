use super::*;

/// One scenario over all seeds, summarised by the statistics the assertions use.
#[derive(Debug)]
pub struct Summary {
    pub name: String,
    pub limit: u32,
    pub mean_target_median: f64,
    pub p10_target_worst: u32,
    pub below_half_p90: f64,
    pub queue_p95_p90: u32,
    pub delivered_median: f64,
    pub frame_age_p95_p90: u32,
    pub has_restore: bool,
    /// Slowest sustained recovery, `None` when any seed never recovered.
    pub recovery_worst_ms: Option<u32>,
    pub cold_start_min_median: u32,
    /// Slowest time to 90% of the limit, `None` when any seed never got there.
    pub time_to_90pct_worst_ms: Option<u32>,
}

impl Summary {
    pub fn of(reports: &[Report]) -> Self {
        let f = |g: fn(&Report) -> f64| reports.iter().map(g).collect::<Vec<_>>();
        let u = |g: fn(&Report) -> u32| reports.iter().map(g).collect::<Vec<_>>();
        let all = |g: fn(&Report) -> Option<u32>| {
            reports
                .iter()
                .map(g)
                .try_fold(0, |worst, ms| ms.map(|ms| worst.max(ms)))
        };
        Summary {
            name: reports[0].name.clone(),
            limit: reports[0].limit,
            mean_target_median: percentile_f64(&f(|r| r.mean_target_fps), 0.5),
            p10_target_worst: percentile_u32(&u(|r| r.p10_target_fps), 0.0),
            below_half_p90: percentile_f64(&f(|r| r.below_half_pct), 0.9),
            queue_p95_p90: percentile_u32(&u(|r| r.queue_p95_ms), 0.9),
            delivered_median: percentile_f64(&f(|r| r.delivered_fps), 0.5),
            frame_age_p95_p90: percentile_u32(&u(|r| r.frame_age_p95_ms), 0.9),
            has_restore: reports[0].has_restore,
            recovery_worst_ms: all(|r| r.recovery_ms),
            cold_start_min_median: percentile_u32(&u(|r| r.cold_start_min_fps), 0.5),
            time_to_90pct_worst_ms: all(|r| r.time_to_90pct_ms),
        }
    }

    pub const HEADER: &'static str = "| scenario | limit | target fps (median of means) | worst p10 | below limit/2 (p90) | queue p95 (p90) | delivered fps (median) | frame age p95 (p90) | sustained recovery (worst) | cold-start min (median) | time to 90% (worst) |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|";

    pub fn row(&self) -> String {
        let secs = |ms: Option<u32>| {
            ms.map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
                .unwrap_or_else(|| "never".to_owned())
        };
        format!(
            "| {} | {} | {:.1} | {} | {:.1}% | {} ms | {:.1} | {} ms | {} | {} | {} |",
            self.name,
            self.limit,
            self.mean_target_median,
            self.p10_target_worst,
            self.below_half_p90,
            self.queue_p95_p90,
            self.delivered_median,
            self.frame_age_p95_p90,
            if self.has_restore {
                secs(self.recovery_worst_ms)
            } else {
                "-".to_owned()
            },
            self.cold_start_min_median,
            secs(self.time_to_90pct_worst_ms),
        )
    }
}

pub(super) fn clean_link(capacity_kbps: f64) -> Link {
    Link {
        capacity_kbps: vec![(0, capacity_kbps)],
        wobble: 0.05,
        base_rtt_ms: 15.0,
        jitter_median_ms: 3.0,
        jitter_sigma: 0.5,
        loss_per_s: 0.0,
        stall_mean_interval_s: 0.0,
        stall_ms: (0.0, 0.0),
    }
}

/// Weak-signal home Wi-Fi with ample average capacity: heavy-tailed jitter,
/// retransmissions, and a link stall of up to 2.5 s every twenty seconds or so.
/// Deliberately nasty; it isolates "capacity is fine, timing is not".
pub(super) fn home_wifi_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 20_000.0)],
        wobble: 0.5,
        base_rtt_ms: 8.0,
        jitter_median_ms: 15.0,
        jitter_sigma: 1.0,
        loss_per_s: 0.2,
        stall_mean_interval_s: 20.0,
        stall_ms: (300.0, 2500.0),
    }
}

pub(super) fn intercontinental_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 20_000.0)],
        wobble: 0.1,
        base_rtt_ms: 250.0,
        jitter_median_ms: 5.0,
        jitter_sigma: 0.5,
        loss_per_s: 0.05,
        stall_mean_interval_s: 0.0,
        stall_ms: (0.0, 0.0),
    }
}

/// 8 Mbps for a minute, 2.5 Mbps for the next, 8 Mbps again.
pub(super) fn halved_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 8_000.0), (60_000, 2_500.0), (120_000, 8_000.0)],
        ..clean_link(8_000.0)
    }
}

/// A cloud relay whose egress is `capacity` times the Balanced bitrate: a stable
/// 40 ms path with a little jitter, nothing else wrong with it.
pub(super) fn relay_link(capacity: f64) -> Link {
    Link {
        base_rtt_ms: 40.0,
        jitter_median_ms: 5.0,
        ..clean_link(BASE_KBPS * Quality::Balanced.ratio() as f64 * capacity)
    }
}

pub(super) fn mobile_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 6_000.0)],
        wobble: 0.4,
        base_rtt_ms: 40.0,
        jitter_median_ms: 30.0,
        jitter_sigma: 0.8,
        loss_per_s: 0.02,
        stall_mean_interval_s: 0.0,
        stall_ms: (0.0, 0.0),
    }
}
