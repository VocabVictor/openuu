//! Relative end-to-end video latency from the sender's frame `pts`
//! (docs/perf: P0-7). The first decoded frame calibrates `(local, pts)`;
//! every later frame's lag is `(local - local0) - (pts - pts0)`, so the value
//! includes capture, encode, queueing, network and decode, but not the
//! clock offset between the two machines. Only the distribution matters.
//! Logged once a second as `qos_e2e`, gated like the server's `qos_*` lines.

use base::message_proto::{video_frame, VideoFrame};
use hbb_common::log;
use std::time::{Duration, Instant};

fn verbose() -> bool {
    static VERBOSE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VERBOSE.get_or_init(|| std::env::var("RUSTDESK_QOS_VERBOSE").is_ok())
}

/// The `pts` of the last encoded frame in `vf`, if it carries one.
pub(super) fn frame_pts(vf: &VideoFrame) -> Option<i64> {
    use video_frame::Union::*;
    let frames = match vf.union.as_ref()? {
        Vp8s(f) | Vp9s(f) | Av1s(f) | H264s(f) | H265s(f) => f,
        _ => return None,
    };
    frames.frames.last().map(|f| f.pts)
}

#[derive(Default)]
pub(super) struct E2eLag {
    origin: Option<(Instant, i64)>,
    last_pts: i64,
    samples: Vec<i64>,
    window_start: Option<Instant>,
}

const WINDOW: Duration = Duration::from_secs(1);

impl E2eLag {
    pub(super) fn observe(&mut self, display: usize, pts: Option<i64>) {
        if !verbose() {
            return;
        }
        let Some(pts) = pts else { return };
        self.observe_at(display, pts, Instant::now());
    }

    fn observe_at(&mut self, display: usize, pts: i64, now: Instant) {
        // A sender restart (display switch, reconnect) starts its pts from
        // zero again; the sender's clock never runs backwards otherwise.
        let restarted = pts < self.last_pts;
        self.last_pts = pts;
        let lag = match self.origin {
            Some((t0, p0)) if !restarted => (now - t0).as_millis() as i64 - (pts - p0),
            _ => {
                self.origin = Some((now, pts));
                self.samples.clear();
                self.window_start = Some(now);
                0
            }
        };
        self.samples.push(lag);
        let start = *self.window_start.get_or_insert(now);
        if now - start >= WINDOW {
            if let Some(line) = summary(display, &mut self.samples) {
                log::info!("{line}");
            }
            self.samples.clear();
            self.window_start = Some(now);
        }
    }
}

fn summary(display: usize, samples: &mut [i64]) -> Option<String> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    // nearest-rank percentile: the smallest value at least q of the samples fall under
    let at = |q: f64| samples[((samples.len() as f64 * q).ceil() as usize).max(1) - 1];
    Some(format!(
        "qos_e2e t={} display={display} n={} p50={} p95={} max={}",
        hbb_common::get_time(),
        samples.len(),
        at(0.5),
        at(0.95),
        samples[samples.len() - 1]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lag_is_relative_to_the_first_frame() {
        let mut lag = E2eLag::default();
        let t0 = Instant::now();
        lag.observe_at(0, 1000, t0);
        lag.observe_at(0, 1033, t0 + Duration::from_millis(40));
        lag.observe_at(0, 1066, t0 + Duration::from_millis(96));
        assert_eq!(lag.samples, vec![0, 7, 30]);
    }

    #[test]
    fn a_sender_restart_recalibrates() {
        let mut lag = E2eLag::default();
        let t0 = Instant::now();
        lag.observe_at(0, 5000, t0);
        lag.observe_at(0, 12, t0 + Duration::from_millis(33));
        assert_eq!(lag.origin.map(|(_, p)| p), Some(12));
        assert_eq!(lag.samples, vec![0]);
        lag.observe_at(0, 45, t0 + Duration::from_millis(70));
        assert_eq!(lag.samples, vec![0, 4]);
    }

    #[test]
    fn summary_reports_percentiles() {
        let mut s: Vec<i64> = (1..=20).rev().collect();
        let line = summary(1, &mut s).unwrap();
        assert!(line.contains(" display=1 n=20 p50=10 p95=19 max=20"), "{line}");
        assert!(summary(0, &mut []).is_none());
    }
}
