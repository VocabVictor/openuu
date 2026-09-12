use super::*;

// Invariant 2: bad evidence never raises a target or the ratio.
#[test]
pub(super) fn bad_evidence_never_raises_a_target_or_the_ratio() {
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
                let ratio_before = qos.ratio();
                let before = match step {
                    Step::Reply { id, .. } | Step::Timeout { id, .. } => Some((id, target(&qos, id))),
                    _ => None,
                };
                apply(&mut qos, step);
                // Bad by the baseline the controller used for this reply: the reply
                // itself may have relearned it.
                let bad = match step {
                    Step::Reply { id, delay } => baseline(&qos, id)
                        .is_some_and(|base| delay >= base + DELAY_THRESHOLD_150MS),
                    Step::Timeout { .. } => true,
                    _ => false,
                };
                if let (true, Some((id, before))) = (bad, before) {
                    let after = target(&qos, id);
                    assert!(
                        after <= before,
                        "seed {seed} abr {abr} step {step_no} {step:?}: target {before} -> {after}"
                    );
                    assert!(
                        qos.ratio() <= ratio_before,
                        "seed {seed} abr {abr} step {step_no} {step:?}: ratio {ratio_before} -> {}",
                        qos.ratio()
                    );
                }
            }
        }
    }
}

// Invariant 2 and 6: a timeout tick keeps or lowers the target, whatever it is.
#[test]
pub(super) fn a_timeout_keeps_or_lowers_every_target() {
    for reference in MIN_FPS..=MAX_FPS {
        for elapsed in [2001, 2999, 3000, 3001, 4500, 6001, 9000, 30_000] {
            let mut qos = session(false);
            open(&mut qos, 1);
            qos.user_custom_fps(1, MAX_FPS);
            qos.users.get_mut(&1).unwrap().delay.fps = Some(reference);
            qos.adjust_fps();
            let stream = qos.fps();
            qos.user_delay_response_elapsed(1, elapsed);
            assert!(
                target(&qos, 1) <= reference,
                "{elapsed} ms outstanding at {reference} fps: target {}",
                target(&qos, 1)
            );
            assert!(
                qos.fps() <= stream,
                "{elapsed} ms outstanding at {reference} fps: stream {stream} -> {}",
                qos.fps()
            );
        }
    }
}

// Invariant 6: the late reply of a braked probe does not brake again.
#[test]
pub(super) fn a_late_reply_after_a_brake_does_not_brake_again() {
    for reference in (MIN_FPS + 1..=MAX_FPS).step_by(3) {
        for elapsed in [2001u32, 3001, 4500, 6001, 9000] {
            for abr in [false, true] {
                let mut qos = session(abr);
                open(&mut qos, 1);
                qos.user_custom_fps(1, MAX_FPS);
                for _ in 0..3 {
                    qos.user_network_delay(1, 20);
                }
                qos.users.get_mut(&1).unwrap().delay.fps = Some(reference);
                qos.user_delay_response_elapsed(1, elapsed as u128);
                let braked = target(&qos, 1);
                qos.user_network_delay(1, elapsed + 100);
                assert_eq!(
                    target(&qos, 1),
                    braked,
                    "abr {abr}, {elapsed} ms outstanding at {reference} fps"
                );
            }
        }
    }
}

/// Viewer 1's target after each of its own events, alone or with company whose
/// events are interleaved: a second viewer with its own replies, timeouts, waits
/// and limits, and a third that joins and leaves along the way.  The display
/// timer is left out: on a dynamic screen it raises the ratio off its floor,
/// and the property holds the bitrate state fixed.
pub(super) fn viewer_one_targets(seed: u64, company: bool, abr: bool, at_floor: bool) -> Vec<u32> {
    let mut own = Driver::new(seed, vec![1]);
    let mut others = Driver::new(seed ^ 0xC0FF_EE, vec![2]);
    let mut qos = session(abr);
    if at_floor {
        qos.ratio = qos.min_ratio();
        sync_bitrate(&mut qos);
        assert!(!qos.can_reduce_bitrate());
    }
    open(&mut qos, 1);
    if company {
        open(&mut qos, 2);
    }
    let no_tick = |step: Step| match step {
        Step::Tick(_) => Step::Wait(1000),
        step => step,
    };
    let mut targets = Vec::new();
    for step_no in 0..STEPS {
        if company {
            for _ in 0..others.rng.below(3) {
                let step = no_tick(others.step());
                apply(&mut qos, step);
            }
            if step_no == STEPS / 3 {
                open(&mut qos, 3);
                others.ids.push(3);
            }
            if step_no == 2 * STEPS / 3 {
                qos.on_connection_close(3);
                others.ids.pop();
            }
        }
        let step = no_tick(own.step());
        apply(&mut qos, step);
        targets.push(target(&qos, 1));
    }
    targets
}
