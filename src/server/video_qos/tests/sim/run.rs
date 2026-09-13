use super::*;

pub fn run(sc: &Scenario) -> Report {
    let mut network_rng = Rng::new(sc.seed);
    let mut probe_rng = Rng::new(sc.seed ^ 0x5052_4F42_45);
    let mut encoder = Encoder {
        model: sc.encoder,
        content: sc.content,
        rng: Rng::new(sc.seed ^ 0x454E_434F_4445),
        next_scene_ms: SCENE_INTERVAL_MS,
        debt_bits: 0.0,
    };
    let total_ms = sc.seconds * 1000;
    let ticks = (total_ms / TICK_MS) as usize;
    let link = link_trace(&sc.link, ticks, &mut network_rng);
    // Probe jitter indexed by the probe's send second, so the number of probes a
    // controller manages to send does not change the jitter the next one meets.
    let probe_jitter_ms: Vec<f64> = (0..=sc.seconds)
        .map(|_| probe_rng.log_normal(sc.link.jitter_median_ms, sc.link.jitter_sigma))
        .collect();

    let mut qos = super::super::smoke::session(sc.limit, sc.quality);
    qos.abr_config = sc.abr;
    if sc.abr {
        qos.new_display("sim".to_owned());
        qos.set_support_changing_quality("sim", true);
    }

    let mut queue: VecDeque<Packet> = VecDeque::new();
    let mut queued_bits = 0.0_f64;
    let mut encode_phase = 0.0_f64;
    let mut encoded_this_second = 0_usize;
    let mut probe_sent: Option<u32> = None;
    // What the send path would see: bits it got out this second, and how much of that
    // second it spent with the path backed up rather than waiting for frames.
    let mut drained_bits_this_second = 0.0_f64;
    let mut blocked_ms_this_second = 0_u32;
    let mut replies: Vec<(u32, u32)> = Vec::new(); // (arrive_ms, delay_ms)
    let restore_ms = sc.link.restore_ms();

    let mut fps_samples = Vec::new();
    let mut queue_samples = Vec::new();
    let mut produced = 0_u64;
    let mut delivered = 0_u64;
    let mut frame_ages = Vec::new();
    let mut trace = Vec::new();
    let mut max_delay = 0_u32;
    let mut recovery_ms = None;
    let mut good_since: Option<u32> = None;
    let mut cold_start_min_fps = u32::MAX;
    let mut time_to_90pct_ms = None;

    for tick in 0..ticks {
        let now = tick as u32 * TICK_MS;
        qos.advance_ms(TICK_MS as u64);
        let capacity_kbps = link.capacity_kbps[tick];

        // Encoder: frames at the controller's rate, sized by the controller's ratio.
        // The video loop reports the bitrate as soon as it applies a new ratio.
        let fps = qos.fps();
        let ratio = qos.ratio();
        let bitrate_kbps = BASE_KBPS * ratio as f64;
        qos.store_bitrate(bitrate_kbps as u32);
        let produce_rate = match sc.content {
            Content::Video => fps as f64,
            Content::Office => (fps as f64).min(2.0),
        };
        encode_phase += produce_rate * TICK_MS as f64 / 1000.0;
        while encode_phase >= 1.0 {
            encode_phase -= 1.0;
            encoded_this_second += 1;
            if now >= WARM_UP_MS {
                produced += 1;
            }
            let bits = encoder.frame_bits(now, bitrate_kbps, produce_rate);
            queue.push_back(Packet {
                bits,
                enqueued_ms: now,
                probe_sent_ms: None,
            });
            queued_bits += bits;
        }

        // Shared path drain: probes are tiny and leave as soon as they reach the head.
        if !link.stalled[tick] {
            let mut budget = capacity_kbps * TICK_MS as f64;
            while budget > 0.0 {
                let Some(head) = queue.front_mut() else { break };
                if let Some(sent) = head.probe_sent_ms {
                    let round_trip = sc.link.base_rtt_ms + probe_jitter_ms[(sent / 1000) as usize];
                    let arrive = now + round_trip as u32;
                    replies.push((arrive, arrive - sent));
                    queue.pop_front();
                    continue;
                }
                let take = budget.min(head.bits);
                head.bits -= take;
                queued_bits -= take;
                budget -= take;
                drained_bits_this_second += take;
                if head.bits <= 1e-9 {
                    if now >= WARM_UP_MS {
                        delivered += 1;
                        frame_ages.push(now - head.enqueued_ms);
                    }
                    queue.pop_front();
                }
            }
        }

        // What a blocked send call means: the socket buffer is full, not that a frame is
        // in flight.  A buffer holds about a quarter second of video.
        if queued_bits / capacity_kbps.max(1.0) > SOCKET_BUFFER_MS {
            blocked_ms_this_second += TICK_MS;
        }

        // Probe replies reach the controller.
        replies.sort_by_key(|r| r.0);
        while replies.first().is_some_and(|r| r.0 <= now) {
            let (_, delay) = replies.remove(0);
            max_delay = max_delay.max(delay);
            probe_sent = None;
            qos.user_network_delay(1, delay);
        }

        // The connection's one second timer.
        if now % 1000 == 0 {
            if probe_sent.is_none() {
                probe_sent = Some(now);
                queue.push_back(Packet {
                    bits: 0.0,
                    enqueued_ms: now,
                    probe_sent_ms: Some(now),
                });
            }
            qos.user_delay_response_elapsed(1, (now - probe_sent.unwrap()) as u128);
            if blocked_ms_this_second >= super::super::super::BLOCKED_MS_FOR_CAPACITY {
                qos.note_link_capacity((drained_bits_this_second / 1000.0) as u32);
            }
            drained_bits_this_second = 0.0;
            blocked_ms_this_second = 0;
            if sc.abr {
                qos.update_display_data("sim", encoded_this_second);
            }
            encoded_this_second = 0;
        }

        if now % 100 == 0 {
            let queue_ms = (queued_bits / capacity_kbps.max(1.0)) as u32;
            let fps = qos.fps();
            trace.push((now, fps, queue_ms, qos.ratio()));
            if now < WARM_UP_MS {
                cold_start_min_fps = cold_start_min_fps.min(fps);
            } else {
                fps_samples.push(fps);
                queue_samples.push(queue_ms);
            }
            if time_to_90pct_ms.is_none() && fps * 10 >= sc.limit * 9 {
                time_to_90pct_ms = Some(now);
            }
            if let Some(restore) = restore_ms {
                if now >= restore && recovery_ms.is_none() {
                    if fps >= sc.limit && queue_ms < 200 {
                        let since = *good_since.get_or_insert(now);
                        if now - since >= SUSTAINED_MS {
                            recovery_ms = Some(since - restore);
                        }
                    } else {
                        good_since = None;
                    }
                }
            }
        }
    }

    let measured_s = (total_ms - WARM_UP_MS) as f64 / 1000.0;
    let below_half = fps_samples.iter().filter(|f| **f * 2 < sc.limit).count();
    Report {
        name: sc.name.to_owned(),
        seed: sc.seed,
        limit: sc.limit,
        mean_target_fps: fps_samples.iter().map(|f| *f as f64).sum::<f64>()
            / fps_samples.len().max(1) as f64,
        p10_target_fps: percentile_u32(&fps_samples, 0.10),
        min_target_fps: fps_samples.iter().copied().min().unwrap_or(0),
        below_half_pct: 100.0 * below_half as f64 / fps_samples.len().max(1) as f64,
        produced_fps: produced as f64 / measured_s,
        delivered_fps: delivered as f64 / measured_s,
        frame_age_p95_ms: percentile_u32(&frame_ages, 0.95),
        queue_p95_ms: percentile_u32(&queue_samples, 0.95),
        max_delay_ms: max_delay,
        has_restore: restore_ms.is_some(),
        recovery_ms,
        cold_start_min_fps,
        time_to_90pct_ms,
        final_fps: qos.fps(),
        final_ratio: qos.ratio(),
        trace,
    }
}
