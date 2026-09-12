use super::*;

pub(in crate::server::video_qos::tests) fn session(fps: u32, quality: Quality) -> VideoQoS {
    let mut qos = VideoQoS {
        fps: INIT_FPS.min(fps),
        abr_config: false,
        ..Default::default()
    };
    qos.advance_ms(2000);
    qos.users.insert(
        1,
        UserData {
            custom_fps: Some(fps),
            quality: Some((0, quality)),
            ..Default::default()
        },
    );
    qos
}

pub(super) fn profiles() -> Vec<(&'static str, Vec<u32>, bool)> {
    vec![
        ("stable_10", vec![10; 120], false),
        ("stable_80", vec![80; 120], false),
        ("stable_180", vec![180; 120], false),
        ("stable_300", vec![300; 120], false),
        (
            "lan_jitter",
            (0..120).map(|i| 10 + (i * 37 % 70)).collect(),
            true,
        ),
        (
            "isolated_spikes",
            (0..120)
                .map(|i| if i % 15 == 0 { 800 } else { 10 })
                .collect(),
            true,
        ),
        ("alternating_10_350", [10, 350].repeat(60), true),
        (
            "two_sample_bursts",
            (0..120)
                .map(|i| if i % 12 < 2 { 700 } else { 10 })
                .collect(),
            true,
        ),
        (
            "threshold_jitter",
            [140, 180, 150, 190, 130, 170].repeat(20),
            true,
        ),
        (
            "congestion_200_recovery",
            [vec![200; 20], vec![10; 60]].concat(),
            true,
        ),
        (
            "congestion_800_recovery",
            [vec![800; 20], vec![10; 60]].concat(),
            true,
        ),
        (
            "congestion_1500_recovery",
            [vec![1500; 20], vec![10; 60]].concat(),
            true,
        ),
        (
            "rising_then_falling",
            (0..80).map(|i| 10 + i.min(79 - i) * 20).collect(),
            true,
        ),
    ]
}

#[test]
pub(super) fn smoke_latency_profiles() {
    use std::fmt::Write;

    let mut csv = String::from("profile,limit,quality,sample,delay_ms,fps\n");
    for (quality_name, quality) in [
        ("balanced", Quality::Balanced),
        ("best", Quality::Best),
        ("low", Quality::Low),
    ] {
        for limit in [1, 5, 15, 30, 60, 120] {
            for (name, delays, warm_up) in profiles() {
                let mut qos = session(limit, quality);
                if warm_up {
                    for _ in 0..90 {
                        qos.user_network_delay(1, 10);
                    }
                    assert_eq!(qos.fps(), limit);
                }
                let mut trace = Vec::new();
                for (i, delay) in delays.into_iter().enumerate() {
                    qos.user_network_delay(1, delay);
                    let fps = qos.fps();
                    assert!((MIN_FPS..=limit).contains(&fps), "{name}: {fps}");
                    trace.push(fps);
                    writeln!(csv, "{name},{limit},{quality_name},{i},{delay},{fps}").unwrap();
                }
                if limit == 30 && quality_name == "balanced" {
                    println!(
                        "{name}: first20={:?}, last={}",
                        &trace[..20],
                        trace.last().unwrap()
                    );
                }
                if name.starts_with("stable_") {
                    assert_eq!(trace.last(), Some(&limit), "{name}, {quality_name}");
                }
                if matches!(
                    name,
                    "lan_jitter"
                        | "isolated_spikes"
                        | "alternating_10_350"
                        | "two_sample_bursts"
                        | "threshold_jitter"
                ) {
                    assert!(
                        trace.iter().all(|fps| *fps == limit),
                        "{name}, {quality_name}"
                    );
                }
                if matches!(name, "congestion_800_recovery" | "congestion_1500_recovery") {
                    assert!(
                        trace.iter().all(|fps| *fps >= limit.min(5)),
                        "automatic reductions must preserve the floor: {trace:?}"
                    );
                    if limit >= 15 {
                        assert!(
                            trace[20] < limit,
                            "a single good reply must not restore the full frame rate"
                        );
                    }
                    assert_eq!(
                        trace[21], limit,
                        "two fresh good replies must restore the frame rate"
                    );
                    if name == "congestion_1500_recovery" {
                        assert_eq!(
                            trace[5],
                            limit.min(5),
                            "severe congestion must brake promptly"
                        );
                    }
                }
                if name == "congestion_200_recovery" && limit >= 15 {
                    assert!(
                        trace[..20].iter().min() < Some(&limit),
                        "moderate sustained congestion must reduce the frame rate: {trace:?}"
                    );
                }
                if name.ends_with("_recovery") {
                    assert_eq!(trace.last(), Some(&limit), "{name}, {quality_name}");
                }
            }
        }
    }
    if let Ok(path) = std::env::var("RUSTDESK_QOS_SMOKE_CSV") {
        std::fs::write(path, csv).unwrap();
    }
}
