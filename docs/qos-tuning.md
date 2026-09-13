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

Fixed separately, in the startup rather than the table: see the next entry.

## 2026-09-13: draining a backlog

The 0.3x row above was the controller being first blind and then powerless. Blind,
because replies drive the bitrate and a deep queue is exactly when they stop arriving:
a stalled probe moved the frame rate but never the bitrate. Powerless, because the
steady-state floor is a rate the link can just carry, and such a rate never wins back
seconds of queue already in front of it.

A viewer now enters a drain on evidence of a deep queue and stays in it until the queue
has gone, asking for one step to under what the link carries, with `BR_MIN_DRAIN` as the
floor and no cooldown. On the same scenario the backlog peaks at 20 s instead of 33 s,
is gone by 31 s instead of never, and the bitrate returns to five times its low with the
frame rate back at the limit. The cost is a few seconds of deliberately poor picture
while it drains, which is the trade the whole entry is about: a thin link can have low
latency or a sharp picture in that moment, not both.

Not addressed here: the first three to six seconds, where nothing is known about the link
and the stream goes out at the preset. See the next entry.

## 2026-09-13: the send path as the first evidence

Probes are the controller's only measurement, and the first one comes back no sooner
than the queue it had to cross. On a link that cannot carry the preset those seconds
were the whole backlog; the drain above was cleaning up after a decision taken blind.

A send that blocks is waiting for the link rather than for frames, so the bits that get
out over a second of blocking are the link. `VideoQoS::note_link_capacity` takes that as
a lower bound, fits the bitrate under it (90 percent of it), downwards only and never
below the steady floor, and only when the path was blocked for half the second. An idle
or frame-limited path reports nothing, which is why no other scenario moved: a link with
headroom never fills the socket buffer.

On the same relay_0_3x_30 scenario, against the drain alone:

| | first cut | peak queue | drained at | steady queue p95 |
| --- | --- | --- | --- | --- |
| drain only | 7 s | 20 s | 31 s | 0.6 s |
| with the send-path evidence | 1 s | 2.2-3.0 s | 7-8 s | 0.4 s |

Against where this started, the backlog a thin link builds went from 33 seconds that
never drained to 3 seconds that are gone by the eighth.

The simulator models the socket the way one behaves, blocking once the buffer holds more
than a quarter second of video. That constant and `BLOCKED_MS_FOR_CAPACITY` are the two
places where this mechanism could be mistuned: too small a buffer and a healthy link
would report a capacity it does not have.
