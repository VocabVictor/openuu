use super::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Content {
    /// Every frame changes: a video call or a movie.
    Video,
    /// Mostly static: a couple of changed frames per second.
    Office,
}

/// How the encoder turns a bitrate into frame sizes.
#[derive(Clone, Copy, PartialEq)]
pub enum EncoderModel {
    /// VP8, VP9 and AV1 run CBR against millisecond timestamps: fewer frames per
    /// second means bigger frames, the bitrate stays.  Only the ratio moves bytes.
    Cbr,
    /// Hardware encoders configured for a fixed 30 fps rate-control assumption:
    /// every frame carries a thirtieth of the bitrate, so fewer frames mean fewer
    /// bytes.  Actual hardware behaviour is backend dependent (Android's MediaCodec
    /// path runs VBR).
    FixedRate,
}

#[derive(Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub seconds: u32,
    pub limit: u32,
    pub quality: Quality,
    pub abr: bool,
    pub content: Content,
    pub encoder: EncoderModel,
    pub link: Link,
    pub seed: u64,
}

/// Bitrate at ratio 1.0; balanced quality (0.67) then encodes at about 4 Mbps.
pub(super) const BASE_KBPS: f64 = 6000.0;
/// The frame rate hardware encoders are configured for.
pub(super) const ENCODER_CONFIGURED_FPS: f64 = 30.0;
/// Log-normal spread of frame sizes around their budget.
pub(super) const FRAME_SIZE_SIGMA: f64 = 0.35;
pub(super) const TICK_MS: u32 = 10;
/// Video a socket buffer holds before a send call blocks.
pub(super) const SOCKET_BUFFER_MS: f64 = 250.0;
/// Samples taken before this instant belong to the cold start, not the steady state.
pub(super) const WARM_UP_MS: u32 = 15_000;
/// A recovery counts once target and queue have held for this long.
pub(super) const SUSTAINED_MS: u32 = 5_000;
/// Seeds every scenario is run with.
pub const SEEDS: std::ops::RangeInclusive<u64> = 1..=20;

/// Frame sizes with a conserved bitrate budget.
pub(super) struct Encoder {
    pub(super) model: EncoderModel,
    pub(super) content: Content,
    pub(super) rng: Rng,
    pub(super) next_scene_ms: u32,
    pub(super) debt_bits: f64,
}

/// A scene change every five seconds of video.
pub(super) const SCENE_INTERVAL_MS: u32 = 5_000;

impl Encoder {
    pub(super) fn frame_bits(&mut self, now_ms: u32, bitrate_kbps: f64, produce_rate: f64) -> f64 {
        let target = match (self.content, self.model) {
            // A changed region of a static screen is small whatever the rate control does.
            (Content::Office, _) => bitrate_kbps * 1000.0 / ENCODER_CONFIGURED_FPS * 0.3,
            (Content::Video, EncoderModel::Cbr) => bitrate_kbps * 1000.0 / produce_rate,
            (Content::Video, EncoderModel::FixedRate) => {
                bitrate_kbps * 1000.0 / ENCODER_CONFIGURED_FPS
            }
        };
        // Mean one: the spread must not change the offered load.
        let noise = self.rng.log_normal(1.0, FRAME_SIZE_SIGMA)
            * (-FRAME_SIZE_SIGMA * FRAME_SIZE_SIGMA / 2.0).exp();
        let mut bits = target * noise;
        // Scene changes follow the wall clock, not the frame count, so every
        // controller meets the same content timeline.
        let scene_change = self.content == Content::Video && now_ms >= self.next_scene_ms;
        if scene_change {
            self.next_scene_ms += SCENE_INTERVAL_MS;
            // A scene change costs a few frames' worth of data; rate control claws
            // it back from the frames that follow.
            bits *= 3.0;
            self.debt_bits += bits - target;
        } else if self.debt_bits > 0.0 {
            let repay = self.debt_bits.min(target * 0.5).min(bits * 0.5);
            bits -= repay;
            self.debt_bits -= repay;
        }
        bits
    }
}

pub(super) struct Packet {
    pub(super) bits: f64,
    pub(super) enqueued_ms: u32,
    pub(super) probe_sent_ms: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub name: String,
    pub seed: u64,
    pub limit: u32,
    /// Controller target, sampled every 100 ms after the warm-up.
    pub mean_target_fps: f64,
    pub p10_target_fps: u32,
    pub min_target_fps: u32,
    /// Share of the measured time the target spent below half of the limit.
    pub below_half_pct: f64,
    /// Frames the encoder produced per second.
    pub produced_fps: f64,
    /// Frames that left the shared path per second.
    pub delivered_fps: f64,
    /// Time a delivered frame spent in the shared path, 95th percentile.
    pub frame_age_p95_ms: u32,
    pub queue_p95_ms: u32,
    pub max_delay_ms: u32,
    /// Whether the link drops and restores its capacity at all.
    pub has_restore: bool,
    /// Time from the capacity restore until target at the limit and queue below
    /// 200 ms held for `SUSTAINED_MS`.
    pub recovery_ms: Option<u32>,
    /// Lowest target during the first `WARM_UP_MS`.
    pub cold_start_min_fps: u32,
    /// First time the target reached 90% of the limit.
    pub time_to_90pct_ms: Option<u32>,
    pub final_fps: u32,
    pub final_ratio: f32,
    pub trace: Vec<(u32, u32, u32, f32)>, // (time_ms, target fps, queue_ms, ratio)
}

pub(super) fn percentile_u32(values: &[u32], p: f64) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[(((sorted.len() - 1) as f64) * p).round() as usize]
}

pub(super) fn percentile_f64(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[(((sorted.len() - 1) as f64) * p).round() as usize]
}
