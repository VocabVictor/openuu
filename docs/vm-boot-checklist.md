# What to run the next time the Hyper-V peer is up

Six verifications were blocked on 2026-09-13 when the virtual machine was powered off to
give its 8 GB back. They are written here in the order to run them, so that whoever has
the machine next runs them without arranging anything first.

## The two constraints the order comes from

**One bundle swap.** Swapping the peer's build is the expensive step, so everything that
wants the old one runs first, then one swap, then everything else. Two swaps means half an
hour of nothing but installing.

**Nothing runs beside anything else.** This peer has two virtual processors. A second
workload -- another session, a build, an ssh that compiles something -- changes every
number on this page, including the ones that look robust. That includes running two items
of this list at once.

**If the machine goes away again before the list is done**, take 3 then 4, 5 and 6: the
swap and the three measurements that nothing else can give. No other machine reproduces
the DXGI condition, and the standby and teardown "after" numbers need this peer on the new
build, where the "before" halves can be taken any time from a bundle that is kept.
Everything else has another way to be believed.

## Before anything

| What | How |
| --- | --- |
| The peer auto-logs on and has an interactive desktop | Section 8 of `docs/perf-baseline-2026-09-13.md`: without one it captures nothing and half this list measures a still screen |
| An inbound firewall rule exists for the peer executable | Otherwise every session falls back to the relay and the connection rows are meaningless (section 1 of the baseline) |
| The installed bundle is known | Swap with the hash-verifying script, and record which commit the build tree was at; a bundle that cannot be named cannot be compared against |

Where a change has a string of its own, grep the built `libopenuu.dll` for it as a second
check that the bundle really contains what the run is about. The DXGI item below has one;
the others are identified by the commit the build came from.

## The list

### 1. Standby, before (old bundle) — e9, 10 min

* **Needs**: the pre-2026-09-13-evening bundle installed, no session connected, nothing
  else running on the machine.
* **Run**: `pwsh -File scripts/perf-baseline.ps1 -PeerKind vm -PeerSsh <peer>` and keep the
  `standby` rows, or the two counters by hand: `\Process(openuu*)\% Processor Time` and
  `\Thread(openuu*)\Context Switches/sec`, 6 samples at 5 s.
* **Decides**: the baseline the five standby commits are measured against.

### 2. Mouse movement, before (counter only) — 52, 10 min

* **Needs**: a controller built at the commit that adds the counter and nothing else. The
  peer can stay on whichever bundle it has, since this measures what the controller sends.
  A window on the peer to drag over.
* **Run**: with `RUSTDESK_INPUT_VERBOSE=1`, one continuous drag of a fixed duration, say
  30 s of circles, and read the counter's mouse-move messages per second.
* **Decides**: the before half of the coalescing comparison. 52 owns the method and chose
  to land the counter as its own commit so that both halves count the same quantity: log
  lines for this do not exist yet, and packet capture after encryption can only count
  bytes. Compare the per-second average, not the total — the drag is done by hand and
  cannot be repeated exactly, so the same person should do both runs at the same pace.

### 3. Swap the peer to a build of current master — e9, 5 min

Everything below needs it. One swap, not one per item.

### 4. Standby, after — e9, 10 min

* **Needs**: same conditions as item 1, nothing connected.
* **Decides**: `de195fc4d` (services sleep until subscribed), `5ee0db0b5` (the child reaper
  waits to be woken), `94b4a7b84` (the service hears about session changes), `970c15fa6`
  (the heartbeat collects only when it can upload), `76ff65ed8` (the tray asks every three
  seconds). Expected direction: context switches per second down by most of what they
  were; about 217 timer wake-ups a second of accounted-for polling became about 6.
* **Note**: this is a peer-side measurement, so the controller bundle does not matter.

### 5. Session teardown — e9, 10 min

* **Needs**: the new bundle, the fixture running so the session does real work.
* **Run**: three samples of threads, handles and private bytes for the three `openuu`
  processes: 60 s idle, 60 s connected, 60 s after disconnecting.
* **Decides**: whether a session gives back what it took. Reading the code says it should
  (the capture loop exits within a frame of the last unsubscribe, the capturer and encoder
  go with it, the audio device is released in the service's reset, per-connection threads
  end with their channels, the connection manager is a separate process that exits). The
  open question is only whether the DXGI and hardware-encoder handles and memory come back,
  which is why this is measured rather than argued. **If it is clean, say so and change
  nothing.**

### 6. DXGI desktop surface mapping — e9 runs it, f0 reads it, 10 min

* **Needs**: the new bundle. e9 has already grepped the built `libopenuu.dll` for
  `cannot be mapped` and `copying it instead` and found both, so `9bdbca7e4` is in the
  bundle that is waiting; install it and run. Three sessions of about 20 s each, then the
  peer log.
* **Background**: this peer logged `dxgi error, fall back to gdi` with
  `Kind(InvalidData)` 28 times, every one of them within 0.08–0.55 s of a session
  starting, which means it spent whole sessions on GDI's full-frame compare and copy. The
  candidate cause is `MapDesktopSurface` being refused because the desktop image is no
  longer in system memory, which a Hyper-V synthetic adapter is exactly the machine to do.
* **Decides**, and all three outcomes are reportable:
  * `dxgi: the desktop surface cannot be mapped` appears **and** `fall back to gdi` is
    gone: confirmed. The peer stays on the duplication for whole sessions from now on, its
    capture CPU should drop, and anything else measured on this peer that day has a second
    variable in it — say so when reporting those numbers.
  * the new line does **not** appear and `fall back to gdi` still does: the cause is
    something else, the change is inert, and f0 goes back to the frame-release path.
  * the new line appears **and** `fall back to gdi` still does: there is a third branch;
    send the surrounding log lines.

### 7. Mouse movement, after — 52, 10 min

* **Needs**: a controller built at the counter commit plus the coalescing change; the same
  drag pattern, the same duration and the same counter as item 2.
* **Decides**: message count down, and the drag still feels continuous — the risk is losing
  intermediate positions, which matters for drawing applications.

### 8. Hole punching, before and after — e9, 20 min

* **Needs**: the controller bundle only, so this can run at any point once item 3 is done
  and does not constrain the order. All three switches are read on the controller side
  (`src/client/start.rs`, `transport.rs`, `webrtc_bridge.rs`); the peer cannot read them
  because they live in `LocalConfig`, which the user-interface process writes and never
  synchronises over the inter-process channel. The answering path says so in a comment of
  its own rather than reading them.
* **Run**: sessions with the defaults as they ship, then with the change, reading the
  controller log for `Hole Punched` and `used to establish`.
* **Decides**: whether "default on, fall back when the probe fails" reaches direct where
  "off unless the server is public" reached the relay.

### 9. Connection manager and session window, screenshots — 52 or 8f, 15 min

* **Needs**: the new bundle and a live session; a person to look at them.
* **Run**: one screenshot per surface, as the restyle work has been doing.
* **Decides**: whether the restyled surfaces look right against a real session rather than
  a mock. Last, because it is interactive and holds the machine.

## Running order, and why

1 and 2 first because they are the only two that want the old bundle. Then one swap (3),
then everything else. Within the new-bundle group, the measurements that need **no**
session (4) come before the ones that need one (5, 6, 7, 9), so that a session left open
by mistake cannot contaminate a standby number. Item 8 sits where its build lands.

About 1 hour 40 minutes of machine time in total, including the swap and the settling
between runs, with nothing else on the machine while any of it runs.
