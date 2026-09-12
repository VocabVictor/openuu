//! Closed-loop network simulation for the QoS controller.
//!
//! The controller is driven the way `Connection` drives it: one TestDelay probe per
//! second, a single probe outstanding, `user_delay_response_elapsed` on every timer
//! tick, `update_display_data` once per second.  Video frames and probes share one
//! FIFO, which stands for the downstream shared path (stream, transport, link): the
//! probe measures the bytes that were handed to that path in front of it.  It is not
//! the server's `tx_video` channel, which the probe does not pass through, and the
//! model does not stall the timer while a send is blocked, as the real loop does.
//!
//! Three independent random streams keep an A/B comparison paired: the network
//! trace (capacity wobble, stalls, loss events) is generated before the run from the
//! network stream alone, probe jitter is a per-second table from its own stream,
//! and scene changes follow the wall clock, so two controllers with the same seed
//! face the same link, the same jitter and the same content timeline whatever they
//! decide.  Only the frame size noise depends on how many frames were produced.
//!
//! The encoder model conserves its bitrate budget: a scene change costs three
//! frames' worth of data and the surplus is repaid by the following frames, so the
//! long-term offered load does not depend on the frame rate under CBR.
//!
//! It still is a model, not a network: it does not reproduce a real transport's
//! congestion control or a real encoder.  Its job is to show how the controller
//! reacts to the *kind* of behaviour a home Wi-Fi, a stable relay or a saturated
//! uplink produce, deterministically and over many seeds.
//!
//! Against overfitting: the CI run uses seeds 1 to 20; `robustness.rs` applies the
//! same bounds to seeds 21 to 120 and sweeps the scenario parameters.  Scenario
//! parameters are educated guesses until a recorded `qos_trace` calibrates them.
use super::*;

mod link;
pub use link::*;
mod scenario;
pub use scenario::*;
mod run;
pub use run::*;
mod summary;
pub use summary::*;

pub fn scenarios() -> Vec<Scenario> {
    let base = |name, limit, link, abr, encoder| Scenario {
        name,
        seconds: 180,
        limit,
        quality: Quality::Balanced,
        abr,
        content: Content::Video,
        encoder,
        link,
        seed: 1,
    };
    use EncoderModel::*;
    vec![
        base("home_wifi_30", 30, home_wifi_link(), true, Cbr),
        base("home_wifi_60", 60, home_wifi_link(), true, Cbr),
        base(
            "home_wifi_fixed_rate_30",
            30,
            home_wifi_link(),
            true,
            FixedRate,
        ),
        base("home_wifi_no_abr_30", 30, home_wifi_link(), false, Cbr),
        Scenario {
            content: Content::Office,
            ..base("office_home_wifi_30", 30, home_wifi_link(), true, Cbr)
        },
        base("city_relay_30", 30, clean_link(50_000.0), true, Cbr),
        base("city_relay_60", 60, clean_link(50_000.0), true, Cbr),
        base(
            "intercontinental_30",
            30,
            intercontinental_link(),
            true,
            Cbr,
        ),
        base("bandwidth_halved_30", 30, halved_link(), true, Cbr),
        base(
            "bandwidth_halved_fixed_rate_30",
            30,
            halved_link(),
            true,
            FixedRate,
        ),
        base(
            "bandwidth_halved_fixed_rate_no_abr_30",
            30,
            halved_link(),
            false,
            FixedRate,
        ),
        base("bandwidth_halved_no_abr_30", 30, halved_link(), false, Cbr),
        base("mobile_bufferbloat_30", 30, mobile_link(), true, Cbr),
    ]
}

/// Runs every scenario over `SEEDS` and returns the per-scenario summaries.
pub fn run_all() -> Vec<(Summary, Vec<Report>)> {
    scenarios()
        .iter()
        .map(|sc| {
            let reports: Vec<Report> = SEEDS
                .map(|seed| run(&Scenario { seed, ..sc.clone() }))
                .collect();
            (Summary::of(&reports), reports)
        })
        .collect()
}

fn write_traces(results: &[(Summary, Vec<Report>)]) {
    use std::fmt::Write;
    if let Ok(path) = std::env::var("RUSTDESK_QOS_SIM_CSV") {
        let mut csv = String::from("scenario,seed,time_ms,target_fps,queue_ms,ratio\n");
        for (_, reports) in results {
            for report in reports {
                for (t, fps, queue, ratio) in &report.trace {
                    writeln!(
                        csv,
                        "{},{},{t},{fps},{queue},{ratio:.3}",
                        report.name, report.seed
                    )
                    .unwrap();
                }
            }
        }
        std::fs::write(path, csv).unwrap();
    }
}

#[test]
fn sim_scenarios() {
    let results = run_all();
    println!("{}", Summary::HEADER);
    for (summary, _) in &results {
        println!("{}", summary.row());
    }
    if std::env::var("RUSTDESK_QOS_SIM_VERBOSE").is_ok() {
        for (_, reports) in &results {
            for r in reports {
                println!(
                    "{} seed {}: target mean {:.1} p10 {} min {} below-half {:.1}% delivered {:.1} age p95 {} queue p95 {} max probe {} recovery {:?} cold-start min {} t90 {:?}",
                    r.name, r.seed, r.mean_target_fps, r.p10_target_fps, r.min_target_fps,
                    r.below_half_pct, r.delivered_fps, r.frame_age_p95_ms, r.queue_p95_ms,
                    r.max_delay_ms, r.recovery_ms, r.cold_start_min_fps, r.time_to_90pct_ms
                );
            }
        }
    }
    write_traces(&results);
    for (summary, _) in &results {
        let violations = bound_violations(summary);
        assert!(
            violations.is_empty(),
            "{}: {violations:?}\n{summary:?}",
            summary.name
        );
    }
}

/// The bounds every scenario summary has to meet, shared by the CI run over `SEEDS`
/// and by the held-out run in `robustness.rs`.  They state what the product needs,
/// not what one seed produced.  If a new seed or a new scenario violates a bound,
/// change the design or loosen the bound with a written reason; never tune a
/// controller constant until the bound passes.
pub fn bound_violations(s: &Summary) -> Vec<&'static str> {
    let name = s.name.as_str();
    let limit = s.limit as f64;
    let mut v = Vec::new();
    let mut check = |ok: bool, what: &'static str| {
        if !ok {
            v.push(what);
        }
    };
    if name.starts_with("home_wifi") || name.starts_with("office_home_wifi") {
        // A jittery but healthy link must stay fast: the whole point of the change.
        // The target rarely leaves the limit, never collapses, and stalls of up to
        // 2.5 s leave about a second of queue at worst.
        check(
            s.mean_target_median >= 0.85 * limit,
            "median target below 85%",
        );
        check(s.p10_target_worst * 3 >= s.limit, "worst p10 below a third");
        check(s.below_half_p90 <= 10.0, "below half the limit over 10%");
        check(s.queue_p95_p90 < 1000, "queue p95 p90 over 1 s");
        if name != "office_home_wifi_30" {
            check(s.delivered_median >= 0.8 * limit, "delivered below 80%");
        }
        // Frame age is the time a delivered frame spent in the shared path: what a
        // viewer waits for on top of the round trip.  A jittery high-capacity link
        // contains isolated stalls of up to 2.5 s, and the bound is on the p90 of
        // per-seed p95 frame age: isolated stalls are tolerated, but they must not
        // turn into a sustained multi-second backlog.  A regression bound, not a
        // latency target; set from the scenario, not from a run.
        check(s.frame_age_p95_p90 < 1500, "frame age p95 p90 over 1.5 s");
    } else if name.starts_with("city_relay") {
        // A clean link is where the developers test; every seed sits at the limit,
        // and a fresh connection reaches 90% of it within ten seconds.
        check(s.p10_target_worst == s.limit, "left the limit");
        check(s.queue_p95_p90 < 50, "queue on a clean link");
        check(s.frame_age_p95_p90 < 100, "frame age on a clean link");
        check(
            s.time_to_90pct_worst_ms.is_some_and(|ms| ms <= 10_000),
            "cold start over 10 s",
        );
    } else if name == "intercontinental_30" {
        // High but stable RTT is not congestion, not even during the cold start.
        check(
            s.mean_target_median >= 0.9 * limit,
            "median target below 90%",
        );
        check(
            s.cold_start_min_median >= INIT_FPS,
            "cold start below INIT_FPS",
        );
        // Frame age excludes the round trip, so a high RTT earns no allowance.
        check(s.frame_age_p95_p90 < 150, "frame age over 150 ms");
        check(
            s.time_to_90pct_worst_ms.is_some_and(|ms| ms <= 10_000),
            "cold start over 10 s",
        );
    } else if let Some((queue_p95_bound_ms, below_half_bound_pct)) = match name {
        // Real congestion must be detected, drained and recovered from.  With a CBR
        // encoder only the bitrate drains the queue, and three probe replies at one
        // second cadence plus a three second ratio cooldown are needed before a
        // confirmed cut, so a few seconds of queue are inherent there.  Without
        // ABR nothing drains a CBR queue at all, so that combination is reported
        // but not asserted.
        "bandwidth_halved_30" => Some((3000, 40.0)),
        "bandwidth_halved_fixed_rate_30" => Some((2000, 10.0)),
        "bandwidth_halved_fixed_rate_no_abr_30" => Some((3000, 30.0)),
        _ => None,
    } {
        check(s.queue_p95_p90 < queue_p95_bound_ms, "queue p95 p90 bound");
        check(
            s.frame_age_p95_p90 < queue_p95_bound_ms,
            "frame age p95 p90 bound",
        );
        check(s.below_half_p90 <= below_half_bound_pct, "below half bound");
        check(
            s.recovery_worst_ms.is_some_and(|ms| ms <= 20_000),
            "sustained recovery over 20 s",
        );
    } else if name == "mobile_bufferbloat_30" {
        check(s.queue_p95_p90 < 2000, "queue p95 p90 over 2 s");
        check(s.frame_age_p95_p90 < 2000, "frame age p95 p90 over 2 s");
        check(s.below_half_p90 <= 15.0, "below half the limit over 15%");
    }
    v
}

/// Replays `qos_trace` lines through a fresh controller and returns
/// `(time_ms, id, recorded_fps, replayed_fps)` per line.  Open loop, FPS only:
/// the recorded delays do not react to the replayed decisions, the session runs
/// with ABR off, and connections are replayed as separate viewers of one 30 fps
/// balanced session.  Time advances by the wall-clock delta between consecutive
/// lines whatever their connection, or by one second per line when a trace
/// predates the `t=` field.
pub fn replay(text: &str) -> Vec<(u64, i32, u64, u32)> {
    // A present but malformed value is a corrupt trace, not a missing field.
    let field = |line: &str, key: &str| -> Option<u64> {
        line.split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .map(|v| {
                v.parse()
                    .unwrap_or_else(|e| panic!("bad {key}={v:?} in {line:?}: {e}"))
            })
    };
    let mut qos = super::smoke::session(30, Quality::Balanced);
    qos.users.clear();
    let mut last_t: Option<u64> = None;
    let mut now = 0_u64;
    let mut trace = Vec::new();
    for line in text.lines().filter(|l| l.contains("qos_trace")) {
        let id = field(line, "id").unwrap_or(1) as i32;
        qos.users.entry(id).or_default();
        let t = field(line, "t");
        let step = match (t, last_t) {
            (Some(t), Some(prev)) => t.saturating_sub(prev).clamp(1, 10_000),
            _ => 1000,
        };
        if t.is_some() {
            last_t = t;
        }
        now += step;
        qos.advance_ms(step);
        if let Some(elapsed) = field(line, "timeout") {
            qos.user_delay_response_elapsed(id, elapsed as u128);
        } else if let Some(delay) = field(line, "delay") {
            qos.user_delay_response_elapsed(id, 0);
            qos.user_network_delay(id, delay as u32);
        }
        let recorded = field(line, "fps").unwrap_or(0);
        trace.push((now, id, recorded, qos.fps()));
    }
    trace
}

/// Replays the log named by `RUSTDESK_QOS_TRACE` and prints the result.
#[test]
fn replay_recorded_trace() {
    let Ok(path) = std::env::var("RUSTDESK_QOS_TRACE") else {
        return;
    };
    let trace = replay(&std::fs::read_to_string(&path).unwrap());
    println!("time_ms,id,recorded_fps,replayed_fps");
    for (t, id, recorded, replayed) in &trace {
        println!("{t},{id},{recorded},{replayed}");
    }
    let mean = trace.iter().map(|t| t.3 as f64).sum::<f64>() / trace.len().max(1) as f64;
    println!(
        "replayed mean target fps: {mean:.1} over {} lines",
        trace.len()
    );
}

#[test]
fn replay_time_axis_is_shared_across_connections() {
    // Two viewers each log once a second for twenty seconds: twenty seconds of
    // wall clock, not forty.
    let text: String = (0..20)
        .flat_map(|i| {
            [
                format!("qos_trace t={} id=1 delay=10 fps=30\n", 100_000 + i * 1000),
                format!("qos_trace t={} id=2 delay=10 fps=30\n", 100_001 + i * 1000),
            ]
        })
        .collect();
    let trace = replay(&text);
    let elapsed = trace.last().unwrap().0 - trace.first().unwrap().0;
    assert!(
        (19_000..=19_100).contains(&elapsed),
        "replayed {elapsed} ms for 19 s of wall clock"
    );
}

#[test]
fn replay_recorded_trace_is_independent_of_connection_id() {
    let replay = |id: i32| {
        let text: String = (0..20)
            .map(|i| {
                format!(
                    "qos_trace t={} id={id} delay=10 fps=30\n",
                    100_000 + i * 1000
                )
            })
            .collect();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustdesk-qos-replay-{}-{nonce}-{id}.log",
            std::process::id()
        ));
        std::fs::write(&path, text).unwrap();
        // Exercise the real replay entry point without changing other tests' environment.
        let test = format!(
            "{}::replay_recorded_trace",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture", "--test-threads=1"])
            .env("RUSTDESK_QOS_TRACE", &path)
            .output();
        std::fs::remove_file(&path).unwrap();
        let output = output.unwrap();
        assert!(output.status.success(), "replay failed: {output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let id = id.to_string();
        let fps: Vec<u32> = stdout
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split(',').collect();
                if fields.len() == 4 && fields[1] == id {
                    Some(fields[3].parse().unwrap())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(fps.len(), 20, "missing replay samples: {stdout}");
        fps
    };
    let expected = replay(1);
    assert_eq!(expected.last(), Some(&30));
    assert_eq!(replay(1652), expected);
}
