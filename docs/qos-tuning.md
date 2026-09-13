# QoS tuning notes

Decisions on the video QoS controller (`src/server/video_qos/`) that came out of the
closed-loop simulation in `src/server/video_qos/tests/sim/`. Change the controller only
with the simulation and the recovery tests green; they encode these findings.

## 2026-09-13: bitrate recovery pace

The 5/10/15 percent increase steps were clamped to 150 kbps per 3 s window. On the
recovery fixture (Balanced, modeled 6000 kbps, 12 s of 800 ms delay cutting the ratio to
a quarter) the clamp needed about 17 windows (51 s) to get back to 90 percent; letting
the steps compound needs 10 windows (29 s). The clamp was removed
(`tests/adaptation/recovery_pace.rs`). Every other simulation bound still holds.

## 2026-09-13: headroom above the preset, rejected

Raising `MAX_BR_MULTIPLE` from 1.0 to 1.5 (let a clear path lift the bitrate to 1.5 times
the preset's ratio) failed five closed-loop tests: the clean `city_relay` scenarios left
the fps limit and queued over 50 ms, three startup fixtures queued over 4 s or ended
above the preset, and the ABR bandwidth-drop smoke test did not recover. The scenario
capacities are set around the preset, so a controller that probes to 1.5x has to congest
any link with capacity between 1.0x and 1.5x before it backs off, which is the
oscillation the performance review warned about. The ceiling stays at the preset's
ratio; a custom bitrate stays a ceiling too. Better picture quality is to come from the
per-resolution bitrate base table instead, validated with new scenarios at 1.3x and 0.7x
capacity.
