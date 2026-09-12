use super::*;

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

pub(super) fn write_traces(results: &[(Summary, Vec<Report>)]) {
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
pub(super) fn sim_scenarios() {
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
