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

## 2026-09-13: the bitrate base table, and what it costs a thin link

The presets in `libs/scrap/src/common/codec/bitrate.rs` were the pixel count in
thousands, so 1080p asked for 2073 kbps at ratio 1.0 and the default Balanced quality
encoded at 1.4 Mbps no matter how much capacity the link had. The table now says what a
resolution needs when the link can carry it: 1080p 6 Mbps at ratio 1.0, so Balanced
4 Mbps and Best 9 Mbps.

The simulation was already calibrated to the new figures (`BASE_KBPS = 6000`, "balanced
quality then encodes at about 4 Mbps"), so every bound it enforces was tuned against this
table rather than the shipped one. Its scenarios express capacity as a multiple of the
configured bitrate, which is what the three new `relay_*` cases vary:

| Relay egress | Frame rate | Queue p95 | Cold start to 90% |
| --- | --- | --- | --- |
| 1.3x the Balanced bitrate | at the limit | 46 ms | 3.1 s |
| 0.7x | 29.1 of 30 | 640 ms | 16 s |
| 0.3x | the 5 fps floor | 32 s | never within the run |

The 0.3x row is the cost of the change. A session starts at the full preset, so a link
that cannot carry it queues until the first confirmed cut three to six seconds later, and
at 0.3x the steady state leaves so little surplus that the startup queue takes minutes to
drain. Raising the table moves the band where this happens from below roughly 0.4 Mbps to
below roughly 1.2 Mbps of real capacity. The controller stays at its floor and keeps
serving, so nothing collapses, but the first half minute on such a link is worse than it
was.

Worth fixing separately, in the startup rather than the table: the controller has no
capacity estimate before the first probes come back, and the fix is to drain below the
steady-state target once the queue is known to be large, not to configure a bitrate no
screen needs.
