use super::*;

#[derive(Default, Debug, Clone)]
pub(super) struct RttCalculator {
    pub(super) baseline: Option<u32>,
    pub(super) samples: VecDeque<u32>,
}

impl RttCalculator {
    pub(super) const WINDOW_SAMPLES: usize = 20;
    pub(super) const MAX_INCREASE_MS: u32 = 50;

    pub fn update(&mut self, delay: u32) {
        if self.samples.len() >= Self::WINDOW_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(delay);
        let baseline = self.baseline.unwrap_or(delay).min(delay);
        self.baseline = Some(baseline);

        if self.samples.len() < Self::WINDOW_SAMPLES {
            return;
        }
        let half = Self::WINDOW_SAMPLES / 2;
        let older_min = self.samples.iter().take(half).min().copied();
        let recent_min = self.samples.iter().skip(half).min().copied();
        let (Some(older_min), Some(recent_min)) = (older_min, recent_min) else {
            return;
        };
        // A rising floor can be a growing queue. Allow 10 ms of probe granularity,
        // but wait for it to settle before forgetting the old baseline.
        if recent_min > older_min.saturating_add(10) {
            return;
        }
        let rise = older_min.min(recent_min).saturating_sub(baseline);
        if rise > 0 {
            self.baseline = Some(baseline + (rise / 2).clamp(1, Self::MAX_INCREASE_MS));
        }
    }

    pub fn get_rtt(&self) -> Option<u32> {
        self.baseline
    }
}
