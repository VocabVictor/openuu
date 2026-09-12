use super::*;

/// xorshift64* generator, so the tests need no external crate and stay reproducible.
pub(super) struct Rng(pub(super) u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        Rng((seed ^ 0x9E37_79B9_7F4A_7C15).max(1))
    }

    pub(super) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    pub(super) fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(super) fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    pub(super) fn normal(&mut self) -> f64 {
        let u1 = (1.0 - self.uniform()).max(1e-12);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    pub(super) fn log_normal(&mut self, median: f64, sigma: f64) -> f64 {
        median * (sigma * self.normal()).exp()
    }

    pub(super) fn exponential(&mut self, mean: f64) -> f64 {
        -mean * (1.0 - self.uniform()).max(1e-12).ln()
    }
}

#[derive(Clone)]
pub struct Link {
    /// Step schedule `(from_ms, kbps)`, sorted by time.
    pub capacity_kbps: Vec<(u32, f64)>,
    /// Slow random walk of the capacity, as a fraction of the nominal value.
    pub wobble: f64,
    pub base_rtt_ms: f64,
    /// Log-normal jitter added to every probe round trip.
    pub jitter_median_ms: f64,
    pub jitter_sigma: f64,
    /// Loss events per second.  A reliable stream turns a loss into a 200-400 ms
    /// retransmission stall followed by a second at half rate.
    pub loss_per_s: f64,
    /// Mean interval between link stalls in seconds, `0` for none.
    pub stall_mean_interval_s: f64,
    /// Uniform stall duration range in milliseconds.
    pub stall_ms: (f64, f64),
}

impl Link {
    pub(super) fn capacity_at(&self, now_ms: u32) -> f64 {
        self.capacity_kbps
            .iter()
            .rev()
            .find(|(from, _)| *from <= now_ms)
            .map(|(_, kbps)| *kbps)
            .unwrap_or(self.capacity_kbps[0].1)
    }

    /// Time at which the capacity was last restored to its initial value, if it ever dropped.
    pub(super) fn restore_ms(&self) -> Option<u32> {
        let initial = self.capacity_kbps[0].1;
        let mut dropped = false;
        for (from, kbps) in &self.capacity_kbps {
            if *kbps < initial {
                dropped = true;
            } else if dropped && *kbps >= initial {
                return Some(*from);
            }
        }
        None
    }
}

/// Everything the link does during a run, decided before the run starts.
pub(super) struct LinkTrace {
    pub(super) capacity_kbps: Vec<f64>, // per tick, wobble and retransmission backoff applied
    pub(super) stalled: Vec<bool>,      // per tick
}

pub(super) fn mark(flags: &mut [bool], from_ms: f64, to_ms: f64) {
    let from = (from_ms / TICK_MS as f64).max(0.0) as usize;
    let to = ((to_ms / TICK_MS as f64).ceil() as usize).min(flags.len());
    for flag in flags.iter_mut().take(to).skip(from) {
        *flag = true;
    }
}

pub(super) fn link_trace(link: &Link, ticks: usize, rng: &mut Rng) -> LinkTrace {
    let mut capacity_kbps = vec![0.0; ticks];
    let mut stalled = vec![false; ticks];
    let mut backoff = vec![false; ticks];
    let mut wobble = 0.0_f64;
    for (i, capacity) in capacity_kbps.iter_mut().enumerate() {
        let now = i as u32 * TICK_MS;
        if now % 100 == 0 {
            wobble = (wobble + rng.normal() * 0.03).clamp(-link.wobble, link.wobble);
        }
        *capacity = link.capacity_at(now) * (1.0 + wobble);
    }
    if link.stall_mean_interval_s > 0.0 {
        let mut start = rng.exponential(link.stall_mean_interval_s) * 1000.0;
        while start < (ticks as f64) * TICK_MS as f64 {
            let len = rng.range(link.stall_ms.0, link.stall_ms.1);
            mark(&mut stalled, start, start + len);
            start += rng.exponential(link.stall_mean_interval_s) * 1000.0;
        }
    }
    if link.loss_per_s > 0.0 {
        let per_tick = link.loss_per_s * TICK_MS as f64 / 1000.0;
        for i in 0..ticks {
            if rng.uniform() < per_tick {
                let start = (i as u32 * TICK_MS) as f64;
                let len = rng.range(200.0, 400.0);
                mark(&mut stalled, start, start + len);
                mark(&mut backoff, start + len, start + len + 1000.0);
            }
        }
    }
    for (capacity, backoff) in capacity_kbps.iter_mut().zip(&backoff) {
        if *backoff {
            *capacity *= 0.5;
        }
    }
    LinkTrace {
        capacity_kbps,
        stalled,
    }
}
