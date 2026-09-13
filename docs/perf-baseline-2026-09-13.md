# Performance baseline, 2026-09-13

Companion to the performance review (P0 items). Every number here comes from a
command-line session (`openuu.exe --connect <id> --password <pw>` for a fixed
number of seconds) and from log assertions; no GUI automation. Addresses and
ids are placeholders.

## Setup

| Role | Machine | Notes |
| --- | --- | --- |
| Controller | Windows 11 laptop, Intel iGPU, 192.0.2.10 | portable bundle of the commit under test |
| Peer A | mini PC, Intel iGPU, 1920x1080, 192.0.2.20, id `123456789` | service registered by hand, no console session (login screen) |
| Peer B | Hyper-V VM, 2 vCPU, no GPU, 1920x1080, 198.51.100.11, id `987654321` | temporary admin account auto-logged on; a scheduled task in the interactive session runs a maximized console that scrolls random numbers |
| Server | self-hosted hbbs/hbbr, 203.0.113.5 | |

Peer diagnostics: machine environment `RUSTDESK_QOS_VERBOSE=1`, service
restarted; the `qos_video` / `qos_send` lines are aggregated per second.
Controller diagnostics: the same variable plus the `qos_e2e` line
(`src/client/e2e_lag.rs`).

To put the link under pressure, cap the peer from the Hyper-V host rather
than inside the guest: `Set-VMNetworkAdapter -VMName <vm> -MaximumBandwidth
2000000` (bits per second, `0` lifts it). A per-application
`New-NetQosPolicy` throttle inside the guest does not attach on a
non-domain machine, and reads back with an empty throttle rate. The host cap
covers the guest's SSH as well, so start the session from the controller and
collect the logs after lifting the cap.

## 1. Direct versus relay (the result that matters most)

| Path | Time to establish | Samples |
| --- | --- | --- |
| Relay (before the fix) | 1.19–1.48 s, mean 1.3 s | 8 sessions to peer A |
| Direct, TCP hole punch (after the fix) | 11–20 ms | 4 sessions to peer A |

Every session to peer A had gone through the relay although the TCP hole
punch succeeded on the LAN (`TCP Hole Punched ... = 192.0.2.20:<port>`,
`is_local: true`): the peer's network profile was Public, the firewall
blocked inbound by default and there was no rule for the peer executable, so
`peer address ... timeout: 1000` fired and the client fell back to the relay.
The peer had been registered as a service by hand; the MSI (`AddFirewallRules`
custom action) and `install.rs` both add the rule, a hand-registered peer has
to add it itself. An inbound allow rule for the executable fixes it; the
silent-install checklist now includes "inbound firewall rule exists".

## 2. Codec paths observed

* Peer A -> controller: `used preference: Auto, encoder: H265`, VRAM
  encoder (`hevc_qsv`), `initial quality: 0.67`; controller `create H265
  decoder success` on the RAM hardware path.
* Peer B -> controller: software AV1 (`AomEncoderConfig`), `cpu num: 2, cpu
  usage: 77%, codec thread: 1`; controller AV1 software decoder.

## 3. Peer A, direct, static screen (60 s)

No console session, so the login screen is captured: `captured`/`sent`
average 0.1 frames per second, `wait_max`, `send_max` and `queued` all 0.
Nothing to measure beyond connection set-up until the peer has a session.

## 4. Peer B, relay, scrolling console (60 s, twice)

| Metric | Run 1 | Run 2 |
| --- | --- | --- |
| captured = sent, fps (avg / min / p95 / max) | 10.0 / 5 / 13 / 14 | 9.9 / 6 / 12 / 13 |
| wait_max ms (avg / p95 / max) | 1.1 / 6 / 8 | 1.6 / 10 / 15 |
| send_max ms (avg / max) | 0.0 / 0 | 0.1 / 4 |
| queued (max) | 0 | 0 |
| controller decoded fps (avg) | 10.4 | 10.1 |
| e2e excess, per-second p50 (median) | 51 ms | 56 ms |
| e2e excess, per-second p95 (median / max) | 86 / 177 ms | 93 / 128 ms |

`e2e excess` is the lag of a frame over the smallest lag seen in the session
(sender `pts` versus local clock), so it measures queueing and jitter, not the
absolute glass-to-glass delay. Peer B is CPU bound: 1080p software AV1 on two
vCPUs caps the pipeline at about 10 fps, with nothing queued on the send side.

Peer B sessions go through the relay for a network-topology reason, not a
code one; see section 7.

## 5. UDP NAT test (after server 19bcbd0)

With `enable-udp-punch` on by default the client first sent three
`TestNatRequest` packets without an answer (`success=false`): the OSS hbbs
never handled that request over UDP. After the server fix: `UDP NAT test ...
time=21 ms, packets_sent=2, success=true`, and the punch mode became
`UDP+TCP punch`; on the LAN the TCP connection still wins (12.9 ms).

## 6. Defaults verified in this build

* `enable-udp-punch=Y` takes effect (the private-server rule that forced it
  off only applies to an empty value).
* `allow-d3d-render=Y` takes effect but the VRAM decoder failed on this
  controller with `Failed to get decode context`: the hwcodec cache dated
  from an earlier boot and its adapter LUIDs were stale (fixed by keying the
  cache on the boot as well), and a second, older installed OpenUU service on
  the laptop holds the IPC name, so the portable instance cannot re-probe.
  To be re-measured on a controller without that conflict.

## 7. Mediator punch fix (server fb362d3bf) and why peer B stays on the relay

The fix "an offer this side cannot answer still gets the TCP punch" was
verified on the reachable peer A: the new bundle establishes in ~1.4 ms over
UDP+TCP punch with no relay fallback, so it carries no direct-connect
regression.

Peer B never connects directly, and this is the environment, not the code: it
sits on the Hyper-V internal switch subnet (198.51.100.0/24 placeholder for
172.31.x) behind the host gateway, while the controller is on the physical LAN
(192.0.2.0/24). hbbs sees the two share a public IP, takes the intranet path
and hands the peer's internal address to the controller, but that address is
not routable from the physical LAN (ping and TCP probe both fail), so the
1 s direct attempt times out and the relay takes over. Disabling the peer's
firewall entirely did not change this. A direct session to peer B is
impossible until it shares an L2 segment with the controller.

## 8. A moving-screen run is only valid when the peer actually captures

The per-second `captured` count is the fixture's own check. In a 2 Mbps
capped run intended to load the wait path, 54 of 56 seconds reported
`captured=0`: the scrolling console was not on the captured desktop, so the
session measured a static screen and the capture-side wait was never
exercised. Numbers from such a run say nothing about the code under test.
Before timing anything, confirm `qos_video` shows `captured` at the target
frame rate; only then apply the bandwidth cap and start the clock. The
P0-a/b capture-pacing change therefore rests on its unit tests, not on a
real-machine comparison.

## 9. Open

* Setting `codec-preference` for a measurement means editing that peer's own
  file, `config/peers/<id>.toml` under `[options]`. `Decoder::preference`
  reads `PeerConfig::load(id).options`, and `PeerConfig::load` returns the
  per-peer file or `Default::default()`; the `options` map is never merged
  with the user defaults. Writing the key into the global defaults file, as a
  first attempt here did, leaves the session on `used preference: Auto`.
* Hardware-encoder numbers on a real machine are deferred. P0-e (framerate
  lock) and P0-d (recovery step) rest on the `video_qos` simulation (73 tests)
  and the `set_fps` unit tests for correctness; a real before/after under a
  bandwidth cap needs a GPU peer that composites a moving screen, which
  neither available peer provides: the build machine has the GPU but is
  headless (no viewer, no DXGI frames), the VM composites but has no hardware
  encoder. A cheap fix is an HDMI dummy plug on the build machine so DXGI
  composites headlessly; this group of numbers is to be filled in then.
