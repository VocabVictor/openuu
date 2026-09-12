use super::*;

/// xorshift64*, so the tests need no external crate and stay reproducible.
pub(super) struct Rng(pub(super) u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        Rng((seed ^ 0x9E37_79B9_7F4A_7C15).max(1))
    }

    pub(super) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub(super) fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub(super) fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Step {
    Reply { id: i32, delay: u32 },
    Timeout { id: i32, elapsed: u128 },
    Wait(u64),
    Tick(usize),
    Cap { id: i32, fps: u32 },
}

pub(super) const SEEDS: u64 = 150;
pub(super) const STEPS: usize = 300;

/// Random events for a set of viewers on one link.  A probe that is out stays
/// out until a reply: the connection reports a growing elapsed time every second
/// and the reply that ends the stall carries at least that delay.
pub(super) struct Driver {
    pub(super) rng: Rng,
    pub(super) ids: Vec<i32>,
    pub(super) base_rtt: u32,
    pub(super) outstanding: HashMap<i32, u128>,
}

impl Driver {
    pub(super) fn new(seed: u64, ids: Vec<i32>) -> Self {
        let mut rng = Rng::new(seed);
        let base_rtt = 10 + rng.below(300) as u32;
        Driver {
            rng,
            ids,
            base_rtt,
            outstanding: HashMap::new(),
        }
    }

    pub(super) fn step(&mut self) -> Step {
        let id = self.ids[self.rng.below(self.ids.len() as u64) as usize];
        let roll = self.rng.below(100);
        match roll {
            0..=64 => {
                let mut delay = if roll < 45 {
                    self.base_rtt + self.rng.below(140) as u32
                } else {
                    self.base_rtt + DELAY_THRESHOLD_150MS + self.rng.below(1500) as u32
                };
                if let Some(elapsed) = self.outstanding.remove(&id) {
                    delay = delay.max(elapsed as u32 + self.rng.below(500) as u32);
                }
                Step::Reply { id, delay }
            }
            65..=74 => {
                let elapsed = match self.outstanding.get(&id) {
                    Some(elapsed) => elapsed + 1000,
                    None => 2001 + self.rng.below(1000) as u128,
                };
                self.outstanding.insert(id, elapsed);
                Step::Timeout { id, elapsed }
            }
            75..=89 => Step::Wait(self.rng.below(1500)),
            90..=96 => Step::Tick(self.rng.below(31) as usize),
            _ => Step::Cap {
                id,
                fps: 1 + self.rng.below(60) as u32,
            },
        }
    }
}

/// What `on_connection_open` inserts, without touching the config store.
pub(super) fn open(qos: &mut VideoQoS, id: i32) {
    qos.users.insert(
        id,
        UserData {
            joined_at: Some(qos.now()),
            ..Default::default()
        },
    );
}

pub(super) fn session(abr: bool) -> VideoQoS {
    let mut qos = VideoQoS::default();
    qos.advance_ms(2000);
    qos.abr_config = abr;
    qos.first_reply_adjusts_ratio = true;
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    qos
}

/// The video loop reports the encoder's bitrate as soon as it applies a ratio.
pub(super) fn sync_bitrate(qos: &mut VideoQoS) {
    let target = qos.latest_quality().ratio();
    let ratio = qos.ratio();
    qos.store_bitrate((4000.0 * ratio / target) as u32);
}

pub(super) fn apply(qos: &mut VideoQoS, step: Step) {
    match step {
        Step::Reply { id, delay } => qos.user_network_delay(id, delay),
        Step::Timeout { id, elapsed } => qos.user_delay_response_elapsed(id, elapsed),
        Step::Wait(ms) => qos.advance_ms(ms),
        Step::Tick(encoded) => qos.update_display_data("test", encoded),
        Step::Cap { id, fps } => qos.user_custom_fps(id, fps),
    }
    sync_bitrate(qos);
}

/// The viewer's private target, as the controller reads it before a reply.
pub(super) fn target(qos: &VideoQoS, id: i32) -> u32 {
    let user = &qos.users[&id];
    user.delay.fps.unwrap_or(INIT_FPS.min(user.fps_cap()))
}

pub(super) fn baseline(qos: &VideoQoS, id: i32) -> Option<u32> {
    qos.users[&id].delay.rtt_calculator.get_rtt()
}

/// Everything the controller keeps about a viewer, for change detection.
pub(super) fn snapshot(qos: &VideoQoS, id: i32) -> String {
    format!("{:?}", qos.users[&id])
}

/// The aggregation `adjust_fps` is meant to compute: the slowest viewer's target,
/// INIT_FPS for a viewer without a reply or inside its first second, within the
/// lowest cap.
pub(super) fn expected_stream(qos: &VideoQoS) -> u32 {
    let mut fps = qos
        .users
        .values()
        .map(|u| u.delay.fps.unwrap_or(INIT_FPS))
        .min()
        .unwrap_or(INIT_FPS);
    if qos
        .users
        .values()
        .any(|u| u.joined_at.is_some_and(|j| qos.since(j).as_secs() < 1))
    {
        fps = fps.min(INIT_FPS);
    }
    let cap = qos
        .users
        .values()
        .map(|u| u.fps_cap())
        .min()
        .unwrap_or(FPS);
    fps.clamp(MIN_FPS, cap)
}
