use super::*;

// Invariant 1: another viewer's replies, timeouts, limits, joins and leaves do
// not change a viewer's private target.  With ABR off the bitrate is fixed; with
// ABR on the shared bitrate state is a designed input, so the property is checked
// at the bitrate floor, where it can no longer change.
#[test]
pub(super) fn a_viewers_target_is_independent_of_other_viewers() {
    for seed in 0..SEEDS {
        for (abr, at_floor) in [(false, false), (true, true)] {
            let alone = viewer_one_targets(seed, false, abr, at_floor);
            let with_company = viewer_one_targets(seed, true, abr, at_floor);
            assert_eq!(
                alone, with_company,
                "seed {seed} abr {abr}: viewer 1's targets differ with company"
            );
        }
    }
}

// Invariant 3: a join adds a constraint, a leave removes it, and neither touches
// another viewer's state.
#[test]
pub(super) fn joins_and_leaves_only_change_the_aggregation() {
    for seed in 0..SEEDS {
        for abr in [false, true] {
            let mut qos = session(abr);
            open(&mut qos, 1);
            open(&mut qos, 2);
            let mut driver = Driver::new(seed, vec![1, 2]);
            let mut next_id = 3;
            let mut present: Vec<i32> = Vec::new();
            for step_no in 0..STEPS {
                let step = driver.step();
                apply(&mut qos, step);
                if driver.rng.chance(10) {
                    let others: Vec<i32> = qos.users.keys().copied().collect();
                    let before: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    qos.adjust_fps();
                    let stream = qos.fps();
                    open(&mut qos, next_id);
                    present.push(next_id);
                    driver.ids.push(next_id);
                    next_id += 1;
                    qos.adjust_fps();
                    assert!(
                        qos.fps() <= stream,
                        "seed {seed} abr {abr} step {step_no}: a join raised the stream {stream} -> {}",
                        qos.fps()
                    );
                    assert_eq!(qos.fps(), expected_stream(&qos));
                    let after: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    assert_eq!(
                        before, after,
                        "seed {seed} abr {abr} step {step_no}: a join changed a viewer"
                    );
                } else if !present.is_empty() && driver.rng.chance(10) {
                    let leaving = present.remove(driver.rng.below(present.len() as u64) as usize);
                    driver.ids.retain(|id| *id != leaving);
                    driver.outstanding.remove(&leaving);
                    let others: Vec<i32> = qos.users.keys().copied().filter(|id| *id != leaving).collect();
                    let before: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    qos.on_connection_close(leaving);
                    let after: Vec<String> = others.iter().map(|id| snapshot(&qos, *id)).collect();
                    assert_eq!(
                        before, after,
                        "seed {seed} abr {abr} step {step_no}: a leave changed a viewer"
                    );
                    assert_eq!(
                        qos.fps(),
                        expected_stream(&qos),
                        "seed {seed} abr {abr} step {step_no}: the stream after a leave"
                    );
                }
            }
        }
    }
}

// Invariant 4: the ratio comes down only when a viewer's own evidence asks for
// it, by that viewer's own step, and a newcomer's first reply does not spend the
// evidence again.
#[test]
pub(super) fn a_bitrate_cut_is_owned_by_a_viewers_evidence() {
    let mut cuts = 0;
    for seed in 0..SEEDS {
        let mut qos = session(true);
        let ids: Vec<i32> = (1..=1 + (seed % 3) as i32).collect();
        for id in &ids {
            open(&mut qos, *id);
        }
        let mut driver = Driver::new(seed, ids);
        for step_no in 0..STEPS {
            let step = driver.step();
            let before = qos.ratio();
            apply(&mut qos, step);
            let after = qos.ratio();
            if after >= before {
                continue;
            }
            cuts += 1;
            let asked: Vec<f32> = qos
                .users
                .values()
                .filter_map(|u| u.delay.ratio_reduction())
                .collect();
            assert!(
                !asked.is_empty(),
                "seed {seed} step {step_no} {step:?}: a cut nobody asked for"
            );
            let deepest = asked.iter().copied().fold(1.0_f32, f32::min);
            assert!(
                after >= before * deepest * 0.999,
                "seed {seed} step {step_no} {step:?}: cut {before} -> {after}, deepest step asked {deepest}"
            );
            // A newcomer replying inside the cooldown finds the evidence spent.
            open(&mut qos, 99);
            qos.advance_ms(driver.rng.below(2900));
            qos.user_network_delay(99, driver.base_rtt);
            assert_eq!(
                qos.ratio(),
                after,
                "seed {seed} step {step_no}: a newcomer's first reply spent the evidence again"
            );
            qos.on_connection_close(99);
        }
    }
    assert!(cuts > SEEDS as usize, "only {cuts} cuts across {SEEDS} sessions");
}

// Invariant 5: a reply leaves the target within its cap, and the stream is the
// aggregation of targets, caps and start-up guards after every decision.
#[test]
pub(super) fn targets_stay_within_caps_and_the_stream_is_their_aggregation() {
    for seed in 0..SEEDS {
        for abr in [false, true] {
            let mut qos = session(abr);
            let ids: Vec<i32> = (1..=1 + (seed % 3) as i32).collect();
            for id in &ids {
                open(&mut qos, *id);
            }
            let mut driver = Driver::new(seed, ids);
            for step_no in 0..STEPS {
                let step = driver.step();
                apply(&mut qos, step);
                match step {
                    Step::Reply { id, .. } => {
                        let cap = qos.users[&id].fps_cap();
                        let t = target(&qos, id);
                        assert!(
                            (MIN_FPS..=cap).contains(&t),
                            "seed {seed} abr {abr} step {step_no} {step:?}: target {t} outside [{MIN_FPS}, {cap}]"
                        );
                    }
                    Step::Wait(_) | Step::Cap { .. } => continue,
                    _ => {}
                }
                assert_eq!(
                    qos.fps(),
                    expected_stream(&qos),
                    "seed {seed} abr {abr} step {step_no} {step:?}: the stream is not the aggregation"
                );
            }
        }
    }
}
