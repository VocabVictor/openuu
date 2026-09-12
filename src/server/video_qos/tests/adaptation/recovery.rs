use super::*;

#[test]
pub(super) fn low_capacity_preserves_auto_floor_and_recovers() {
    let mut sc = super::super::sim::scenarios()
        .into_iter()
        .find(|s| s.name == "bandwidth_halved_fixed_rate_no_abr_30")
        .unwrap();
    sc.link.capacity_kbps = vec![(0, 8000.0), (60_000, 700.0), (120_000, 8000.0)];
    for seed in super::super::sim::SEEDS {
        sc.seed = seed;
        let report = super::super::sim::run(&sc);
        assert!(
            report.trace.iter().all(|(_, fps, ..)| *fps >= 5),
            "seed {seed}: automatic reductions went below 5 FPS"
        );
        let congested: Vec<_> = report
            .trace
            .iter()
            .filter(|(t, ..)| (80_000..120_000).contains(t))
            .collect();
        let min_fps = congested.iter().map(|(_, fps, ..)| *fps).min().unwrap();
        let min_queue = congested
            .iter()
            .map(|(_, _, queue, _)| *queue)
            .min()
            .unwrap();
        assert_eq!(
            min_fps, 5,
            "seed {seed}: severe congestion must reach the floor"
        );
        // At 5 FPS this model sends about 670 kbps, leaving little room to drain
        // existing backlog at 700 kbps. Require drainage after capacity returns.
        assert!(
            report.recovery_ms.is_some_and(|ms| ms <= 20_000),
            "seed {seed}: recovery took {:?}",
            report.recovery_ms
        );
        println!("700 kbps fixed-rate, seed {seed}: min FPS={min_fps}, min queue={min_queue} ms, queue p95={} ms, frame age p95={} ms, recovery={:?}", report.queue_p95_ms, report.frame_age_p95_ms, report.recovery_ms);
    }
}

pub(super) const DISPLAY: &str = "adaptation";

pub(super) fn session(abr: bool) -> VideoQoS {
    let mut qos = super::super::smoke::session(FPS, Quality::Balanced);
    qos.abr_config = abr;
    qos.new_display(DISPLAY.to_owned());
    qos.set_support_changing_quality(DISPLAY, true);
    sync_bitrate(&mut qos);
    qos
}

pub(super) fn sync_bitrate(qos: &mut VideoQoS) {
    let bitrate = (6000.0 * qos.ratio()) as u32;
    qos.store_bitrate(bitrate);
}

pub(super) fn second(qos: &mut VideoQoS, delay: u32, dynamic: bool) {
    let encoded = if dynamic { qos.fps() as usize } else { 0 };
    qos.advance_ms(1000);
    sync_bitrate(qos);
    qos.user_network_delay(1, delay);
    sync_bitrate(qos);
    qos.update_display_data(DISPLAY, encoded);
    sync_bitrate(qos);
}

pub(super) fn baseline_steps() -> Vec<String> {
    let mut unmet = Vec::new();
    println!("Baseline step: 90 s at 10 ms, 180 s at new RTT, 90 s at 10 ms; one fresh reply per second. Relearning requires FPS=30 and excess<150 ms throughout the final 60 s at the new RTT.");
    println!("| ABR | new RTT | cold-start final FPS | learned baseline | final excess | final FPS | final ratio | relearned | returned-path FPS |");
    println!("|---|---:|---:|---:|---:|---:|---:|---|---:|");
    for abr in [false, true] {
        for rtt in [310, 410] {
            let mut cold = session(abr);
            for _ in 0..90 {
                second(&mut cold, rtt, true);
                assert!(cold.fps() >= INIT_FPS, "stable cold-start RTT {rtt}");
            }
            assert_eq!(cold.fps(), FPS);
            let mut qos = session(abr);
            for _ in 0..90 {
                second(&mut qos, 10, true);
            }
            assert_eq!(qos.fps(), FPS);
            let mut relearned = true;
            for s in 0..180 {
                second(&mut qos, rtt, true);
                if s >= 120 {
                    let base = qos.users[&1].delay.rtt_calculator.get_rtt().unwrap();
                    relearned &= qos.fps() == FPS && rtt.saturating_sub(base) < 150;
                }
            }
            let base = qos.users[&1].delay.rtt_calculator.get_rtt().unwrap();
            let high_fps = qos.fps();
            let high_ratio = qos.ratio();
            for _ in 0..90 {
                second(&mut qos, 10, true);
            }
            assert_eq!(
                qos.fps(),
                FPS,
                "return to the original path: ABR={abr} RTT={rtt}"
            );
            assert!(qos.ratio() >= BR_BALANCED * 0.95);
            println!("| {abr} | {rtt} ms | {} | {base} ms | {} ms | {high_fps} | {high_ratio:.3} | {relearned} | {} |",
                cold.fps(), rtt.saturating_sub(base), qos.fps());
            if !relearned {
                unmet.push(format!(
                    "ABR={abr}, RTT 10 -> {rtt} ms: base={base}, FPS={high_fps}"
                ));
            }
        }
    }
    unmet
}

#[test]
pub(super) fn baseline_step_relearns_higher_rtt() {
    let unmet = baseline_steps();
    assert!(
        unmet.is_empty(),
        "higher baseline was not relearned: {unmet:?}"
    );
}

#[test]
pub(super) fn static_to_dynamic_ratio_recovery() {
    println!("Static recovery: 90 s healthy video, 12 s confirmed 800 ms delay, 60 s healthy static screen, then 90 s video. Bitrate is modeled as ratio * 6000 kbps.");
    println!("| restart profile | ratio after cut | ratio after static | time to 90% | time to 95% | final ratio | final FPS | modeled bitrate |");
    println!("|---|---:|---:|---|---|---:|---:|---:|");
    for (profile, restart_delay, growing_queue) in [
        ("healthy 10 ms", 10, false),
        ("stable path 800 ms", 800, false),
        ("growing queue 800 + 10 ms/s", 800, true),
    ] {
        let mut qos = session(true);
        for _ in 0..90 {
            second(&mut qos, 10, true);
        }
        let target = qos.latest_quality().ratio();
        assert_eq!(qos.ratio(), target);
        for _ in 0..12 {
            second(&mut qos, 800, true);
        }
        let after_cut = qos.ratio();
        assert!(
            after_cut < target * 0.5,
            "fixture must confirm congestion and cut bitrate"
        );
        for _ in 0..60 {
            second(&mut qos, 10, false);
        }
        let after_static = qos.ratio();
        let mut t90 = (after_static >= target * 0.90).then_some(0);
        let mut t95 = (after_static >= target * 0.95).then_some(0);
        for s in 1..=90 {
            let delay = restart_delay + if growing_queue { s * 10 } else { 0 };
            second(&mut qos, delay, true);
            let ratio = qos.ratio();
            if ratio >= target * 0.90 {
                t90.get_or_insert(s);
            }
            if ratio >= target * 0.95 {
                t95.get_or_insert(s);
            }
            if growing_queue {
                assert!(
                    ratio <= after_static * 1.02,
                    "activity must not restore quality into congestion"
                );
            }
        }
        let seconds = |time: Option<u32>| {
            time.map(|s| format!("{s} s"))
                .unwrap_or_else(|| "never".to_owned())
        };
        let ratio = qos.ratio();
        println!("| {profile} | {after_cut:.3} | {after_static:.3} | {} | {} | {ratio:.3} | {} | {} kbps |",
            seconds(t90), seconds(t95), qos.fps(), qos.bitrate());
        if !growing_queue {
            assert!(
                t95.is_some(),
                "{profile}: video did not regain 95% quality within 90 s"
            );
            assert_eq!(qos.fps(), FPS);
        }
    }
}
