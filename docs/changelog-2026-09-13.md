# Changelog 2026-09-12 – 2026-09-13

Two repositories: `openuu` (client) and `openuu-server` (hbbs / hbbr /
openuu-account). Server hashes are 7 characters, client hashes 9. The server
side was deployed to the Tencent CVM three times on 2026-09-13 (06:07, 06:27
and the third after the zero-account smoke test); the client from master
a84c5c4d1 (bundle EC16A4976D69) is installed on the three test machines.

## Defects

### Server
* **hbbs never answered the client's secure_tcp key exchange**, so a client
  with a key and an account token timed out before sending any punch-hole or
  relay request. hbbs now sends the signed KeyExchange first and encrypts
  both directions once the client answers; old clients stay in clear text.
  `04368cf`, tests `f139f36`.

### Client
* **Controlled peers could not be reached without an account session**:
  the relay ticket was fetched with the peer's own login (`85f5388a6`), and
  the same commit refused every LoginRequest until the peer signed in.
  Ruling: accounts and tickets constrain the controller only. The peer now
  uses the ticket hbbs forwards (`e5418fda2`), asks hbbs for one when it
  initiates the relay itself (`5be984f70`), and the two controlled-side login
  gates are gone (`a84c5c4d1`).
* **Pinned Windows session option was unreadable** when written by the
  settings page: the value is a TOML single-quoted string and the parser
  only stripped double quotes. Parsed as TOML now. `7dfcacd9f`.
* MSI: import the installing user's config before creating the service
  `01042ed3b` (replaces the reverted `bbe6fcc73` / `b732500c5`).
* **master did not compile on Linux** after the split work: seven
  visibility and naming errors in `cfg(linux)` items that the Windows build
  machine cannot see, found by the first run of the new Linux check
  workflow (`968942197`, run 34723224521). Fixed in `1a78914dc`
  (input_service wayland clipboard items `pub(in crate::server)`),
  `a0df9660f` (clipboard_service `wayland` renamed `wayland_text`),
  `810f2f543` (core_main helpers exported), `b29837544` (clipboard context
  `pub(super)`); third run green (34726276600). The workflow is now a gate
  for any change touching Linux/Unix items (AGENTS.md) and also runs on
  pushes to master (`878ab1ab1`).
* Legacy signed-in session migrated to the connection-gate key
  `46d54296b`; portable data.bin include path after the move `fe9c91e8a`;
  audio_state_tests reaching AudioHandler fields `adf96605d`.

## Features

### Server
* **Relay tickets for unattended peers** (docs/relay-ticket-peer.md,
  docs/relay-ticket-peer-initiated.md): the account store mints a second
  single-use ticket per relay uuid (`eb22859`); hbbs puts it in the
  RequestRelay it forwards to the peer (`b3bb231`); for relays the peer
  initiates itself hbbs remembers the controller's session for 60 s after
  the punch request (`dfbdd8e`) and answers the peer's RelayResponse with a
  ticket on the same socket (`6ab02dd`).
* **Relay ticket validation over HTTP** (docs/relay-ticket-http.md), so hbbr
  can run on a host without the account database: internal endpoint
  `POST /api/internal/relay-ticket/redeem` behind `OPENUU_INTERNAL_SECRET`
  (`c7357a6`, tests `48daeb5`); hbbr `Redeemer` selected by
  `OPENUU_ACCOUNT_URL`, https or private-address http only, fail closed
  (`ff6e55f`, tests `4fb4fa1`). Not yet switched on in production.
* **Connection audit storage** `POST /api/audit/conn`: no session (the
  poster is the unattended peer); reporting id must be registered in the
  hbbs peer table (`OPENUU_PEER_DB`), 4 KB cap, 60/min/IP, nonce dedup,
  server-side `from_ip`/`received_at`. `8b115c0`.
* Structured event logs for punch-hole decisions, relay requests and
  responses, relay close with uuid, secure_tcp, peer tickets. `2c04c8e`,
  `4e33f3f`, `79394a2`.
* Offline release build + deploy script for the 1.7 GB CVM with backup,
  health check and rollback: `scripts/build-server-remote.sh` `ce7a360`.
* Earlier on 09-12: OpenUU rebrand of tools and packaging `9e30897`,
  account service and authenticated relay access `67cb1a7`, LAN wake
  requests `c979d3f`, account admin subcommands `ad8b9af`, structured
  security events and health check `08434f4`.

### Client (09-12, before the split work)
* Windows session pinning `f0583dd9b`; shared title bar with account menu
  `f5e68e363`; Windows installer rebrand `59b7468d2`, Rust core shipped as
  libopenuu.dll `6bbe33ff7`; grouped device overview `4a98c8371`, device
  detail page `a0b26a520`, terminal session manager `860636c5f`, quick
  launch `9326657bc`, read-only sessions `5f456df51`, more-tools dialog
  `5ad10ae41`; LAN wake relay `9ad456d91`; batched file sends `c8a4c9de0`;
  remote printing and pipelined file sends removed `7655f7f84`;
  OpenUU branding and isolated app name `2c240d355`, `865cd2277`,
  `98c774929`, `bfdc512c0`, `9d00f0825`.

## Refactoring

* **300-line file rule** (AGENTS.md "File Size Rule"): 425 `refactor`
  commits on the client between 09-12 and 09-13 split every Windows-buildable
  Rust and Dart source file over 300 lines into directory modules, in steps
  of 1–3 files each, every step checked on the build machine
  (`cargo check --lib --features flutter`, `flutter analyze`). Largest:
  `src/server/connection.rs` (7569 lines, 18 steps `157802f86`…`a87814c7a`),
  `src/ui_session_interface`, `src/client`, `src/server/video_qos`,
  `flutter/lib/desktop`, `flutter/lib/models`, `libs/scrap`. Remaining files
  over the limit are either single-function exceptions (AGENTS.md table) or
  Linux/macOS-only (AGENTS.md "Deferred", now getting a Linux CI check
  `.github/workflows/linux-check.yml`).
* Server: `rendezvous_server.rs`, `relay_server.rs`, `account.rs`,
  `common.rs` split into modules `57b4a10`, `9c51c83`, `8848154`, `0381c3d`.
* **Sciter UI removed**: `src/ui`, its cfg branches and packaging paths;
  `flutter` is the default feature. `9260a8fc2`, `b9e720639`, `1d908c2ef`,
  `16a5f882a`, `7d677c70f`.
* **Web build target removed** (mobile stays): lib/web, models/web_model
  and the 13 `dart.library.html` conditional imports (`2060f7d7d`, one
  commit by necessity), the web home/settings widgets (`fb54b803f`), then
  the isWeb/isWebDesktop branches folded directory by directory —
  models/utils `66d1227c5`, common `12cb4f4b5`, desktop `2e722e7fd`,
  common/widgets `7230d0536`, mobile `e03356dab` — and finally the
  platform constants and stubs (this commit's parent). 3 000+ lines
  deleted; flutter analyze 233 issues against the 238 baseline.
* Pre-existing rustc warnings cleared without behaviour change
  (`12a4f1fc8`; lib 0 warnings); `cb3d15f77`; Dart splitter tool
  `a62b92fa4`; test file `input_modifier_utils_test.dart` split
  `21d348f5f`, `4dc3d3100`, `1252eb6ef`.

## Verification

* Build machine (192.0.2.10): every commit above passed `cargo check`
  with the `flutter` feature (client) or `cargo test` (server: lib 34,
  hbbr 11 tests at `8b115c0`); flutter analyze baseline 245 issues.
* CVM smoke tests (openuu-52, docs/smoke-2026-09-13.md): round 3 on
  a84c5c4d1 with both controlled peers signed out — connection through the
  relay, 30 fps video, keyboard/mouse, RDP pinned session; no
  "Sign in to OpenUU" in the peer logs. After the third server deployment
  the audit endpoint stores records for both peers and the 404s are gone.
  The peer-initiated relay path (hbbs answering a RelayResponse with a
  ticket) could not be exercised: with both ends behind the same public IP
  hbbs classifies even a forced relay as local_addr and the controller
  requests the relay through hbbs. It is covered by unit tests on both
  sides and remains to be verified on a symmetric-NAT or proxied peer.

## Known follow-ups

See docs/backlog.md and the "Known limitations" sections of the two relay
ticket designs in openuu-server.

---

# 2026-09-13, afternoon and evening

Everything below landed after the entry above. Client hashes are 9
characters, server hashes 7. Both repositories had their history rewritten
twice during this period, so any hash quoted before those rewrites no longer
resolves; the hashes here are the current ones.

## Defects

* A session window closed while maximized saved the window's offset as its
  size, because the size fell back to the wrong field when nothing was
  stored for that peer yet. The peer then remembered a frame the size of a
  title bar (85x77 here), every later session to that peer restored the
  sliver and saved it again, and the window shrank further each time until
  only its title bar was left. Upstream logic, untouched today, but the
  frame memory added this afternoon writes a peer frame in more cases and
  brought it out: it took four sessions to a test peer to reach the sliver.
  The size now falls back to the window's own size, and a stored frame
  below 320x240 is ignored on restore, so the peers already spoiled by this
  fit themselves to the display again instead of staying broken; a
  configuration written before the fix needs no hand editing. `83403321d`,
  9 window-fit unit tests, `flutter analyze` 223.
* The star and the chevron of a device row drifted right with the length of
  the device name, so no two rows had them in the same column and the local
  row, which has neither, left a ragged gap. The name was a `Flexible` next
  to a `Spacer`, and a `Flex` splits its free space between two flexible
  children: the name took what it needed and the spacer got only half of
  what was left. The name and its badge now take the middle as one
  `Expanded` child and the icons sit on the card's right edge. `084b443ac`,
  a widget test measures the icon centres across three name lengths.

* Two verifications turned out never to have run at all. The Android arms of
  the Rust core had not been compiled since they were written: the first CI run
  of the revived `android-build.yml` found a vector whose element type only
  ever came from the three desktop `cfg` blocks, and a call to a `message()`
  method `DeployResult` has never had. Separately, nothing anywhere ran the 147
  Dart widget tests -- no workflow, and not the build machine either, where a
  proxy variable makes `flutter test` fail to load its suite rather than fail a
  test. A button renamed in `0ec0a3088` had left all nine cases of the
  transfer-layout suite red with no one to notice. `07a0dc2c3`, `aea6320f2`,
  `fc75fab29`, gated from now on by `android-build.yml` and by a `flutter test`
  job in `linux-check.yml` (`1df7a9acb`).
* Escape in a `UiDialog` fell back to the first secondary action when no
  close handler was given. Every call site happened to put the cancelling
  action first, so nothing misbehaved; but a trust dialog puts the permissive
  choice there ("continue anyway"), and Escape would have taken it the moment
  one moved onto the shell. Escape now runs only the explicit close handler.
  Found by the session-window design self-check, not by a report or a crash;
  no external trigger existed. `3249bd8e0`, `flutter analyze` 224 -> 223.
* One hardware encoder failing three times in a row emptied the list of every
  hardware encoder the machine had been probed for, for the rest of the
  process; the failures that get there are mostly temporary (a driver reset,
  an encode session another program wanted) and the only recovery was
  restarting the program, since the negotiation that follows recomputes from
  the now empty list and nothing probes again. A failure is now a note about
  the one encoder that failed, named by codec for RAM and by format, driver
  and adapter for VRAM, forgotten after ten minutes. `5357d9864`, 4 scrap
  unit tests, `cargo check --lib --features flutter,hwcodec,vram` clean.
* Any error from the desktop duplication put the session on GDI for the rest
  of its life, whatever the error was. The one Windows produces most is
  `DXGI_ERROR_ACCESS_LOST`, which is not a failure but what happens when the
  desktop is replaced, and every consent prompt replaces it: in a remote
  assistance session the first prompt cost the session its capture path and
  left it comparing and copying whole frames for every capture after that.
  Errors are now read before they are answered, and a lost duplication is
  made again, three times in a row at most with a growing wait.
  `494063e84`, 9 unit tests on the decision.
* A desktop surface that cannot be mapped is copied instead of ending the
  session's duplication. `DesktopImageInSystemMemory` is what the duplication
  reported when it was created and a driver may keep the image elsewhere
  afterwards; the copy the capturer already uses when the flag was false to
  begin with works either way. A candidate explanation for a Hyper-V peer
  whose logs show `DXGI_ERROR_INVALID_CALL` and a GDI fallback within half a
  second of every session start, 28 times; the refusal is logged, which is
  the evidence for whether it is. Not yet confirmed on that peer: the virtual
  machine was powered off before the build reached it. `9bdbca7e4`.
* The three above are the same mistake in three places, and the lesson is now
  a rule in `AGENTS.md`: a capability that fails degrades that capability,
  never the subsystem that offers it. What made them hard to see is that each
  one answers a real failure, and answers it far too widely; none of them
  looks wrong at the line where it is written.
* A fourth defect was reported alongside them and turned out not to be one:
  the empty `EncodingUpdate::Check` arm, which recomputes the negotiated codec
  from the config and therefore could never bring hardware encoding back. It
  reads as a defect only while something upstream is emptying the config; with
  that fixed, recomputing from an intact config is exactly its job. The
  symptom was left alone and the cause was fixed, and the re-probe that would
  have been added to `Check` instead would have spawned a probe process
  whenever an encoder had a bad minute.
* A cached hardware-codec probe from an earlier boot was still trusted, so
  every VRAM decode context named an adapter that no longer existed and D3D
  decoding fell back to the CPU path with `Failed to get decode context`; the
  probe now records the boot it ran in. `bbb5a0a9d`, scrap unit test, and
  `cargo check --lib --features flutter,hwcodec,vram` clean.
* A WebRTC offer this side cannot answer no longer skips the TCP punch.
  `fb362d3bf`, verified on the LAN peer: direct in about 1.4 ms over
  UDP+TCP punch with no relay fallback.
* hbbs ignored `TestNatRequest` over UDP, which the OSS server never handled,
  so the client's NAT probe always failed and UDP punching could not start.
  `19bcbd0`, deployed, client then logs `success=true` and the punch mode
  becomes `UDP+TCP punch`.
* hbbs closed a TCP registration connection after answering one-shot
  requests. `58f3891`, `cfce1dc`, test `8e2b6ee`.
* A remote window ignored the generic saved frame when its own peer had
  none, fitted to the logical work area, and only remembers a frame after a
  real user resize. `99acc702b`, `9aa881fa6`, `a759a73ed`, log-verified.
* The end-to-end lag instrument imported `message_proto` from the wrong
  crate and mis-detected a sender restart. `0d197948c`, `09d6c2c88`.
* **A peer asked public STUN servers because the other end had a feature on.**
  `test_ipv6` learns this host's public v6 address from the built-in list of
  public STUN servers when the deployment has configured none of its own, and
  two of its three call sites had no gate: the controlled side's `start_ipv6`
  and NAT typing. Which third parties a machine contacted was therefore
  decided by the *other* end's settings, and a controlled machine is usually
  the one whose owner is not watching. The gate now sits at the probe so every
  path is covered; skipping costs nothing, since the host simply offers no v6
  candidate. `ee2ba8721`, with a test that asserts a disabled probe does not
  even record an attempt.
* **Hole punching was switched off for every self-hosted deployment.**
  `get_local_option` answered `N` for the three punch switches whenever the
  rendezvous server was not one of the public ones, reading a deployment's
  shape as a statement about its capability; our own hbbs punches like any
  other, so self-hosted deployments relayed every session while reporting the
  switches as off. What decides the default now is whose servers a capability
  needs when nothing is configured: UDP punching talks only to this
  deployment's own rendezvous server and defaults on, while IPv6 punching and
  WebRTC fall back to the public STUN list and wait for `ice-servers` or an
  explicit switch. `7cace660c`, `1be21e66f`; the reasoning, the interaction
  between the baked-in and read-time layers, and the timeout on each fallback
  path are in `docs/punch-defaults.md`.

  In the shipped default configuration this changes no behaviour, since UDP
  punching was already on through the baked-in default and the other two
  remain off. Measured on the LAN peer: direct in 1.07 ms with an inbound
  rule, relay in 1.21 s without one, so the fallback still costs about the
  second its timeout allows.

## Features

* The end-to-end video lag is measurable for the first time: the client
  reads the sender's `pts`, calibrates on the first decoded frame and logs
  `qos_e2e` once a second as the excess over the best lag seen, gated by the
  existing diagnostics variable. `ec010b0b7`, `b8940747a`, 4 unit tests.
* Windows clients default to `allow-d3d-render=Y` and every client to
  `enable-udp-punch=Y`, installed in the layer a user value still overrides.
  `628ce9ef2`, `202cec560`, 4 unit tests; the private-server rule that forced
  punching off only applies to an empty value, so the default takes effect.
* This device can be shared from the assistance page as a QR code.
  `c8eb34ab7`.
* A generic dialog shell and the dialogs moved onto it: login, connect
  password, 2FA, wait-for-acceptance and relay hint. `4306fd8f9`,
  `f6bb1fa01`, `9bcbfcbc5`, `e05471593`, `f266873bf`.
* A session status bar with its controller and a countdown for a dropped
  session. `d2f925cb4`, `641982bc2`, `d84c98abe`, `a18401c4a`.
* `--page login` opens the login dialog directly. `f069c861c`.
* Settings work: collapsible groups, the licence row with the fingerprint
  under Advanced, Advanced folds on Display and Security, grouped connection
  defaults, and warning styling while the insecure TLS fallback is on.
  `5c8de42c7`, `7541fb2c4`, `bbac9c555`, `f68aef858`, `1a5a8a82c`,
  `35653f5d7`, `c1d7963ce`.
* Session windows, toolbar, tab items and the port-forward, file-transfer
  and terminal pages moved onto the design tokens. `7ea84b1ad`, `5853f5319`,
  `39e12ebf6`, `5980a0f9a`, `cd9a8f279`, `ea6beb38f`, `4242def67`,
  `0ec0a3088`, `50fdcf0b0`. flutter analyze stayed at or below the baseline
  on every step.

## Refactoring and splits

* `Client::_start_inner` gave up its RelayResponse arm and its punch loop and
  is now under 300 lines, so its exception row is gone. `9a48f6289`,
  `ea649aa10`, tests `321a82fc1`, `13101c122`, `b3f944001`.
* `drm_capturer.rs` split in nine steps (`9e3bddb83` to `d77baff59`) and
  `ipc/drm.rs` in seven (`1082eefff` to `3d285e12a`), each step checked on
  the build machine's WSL Debian with the `flutter` and `flutter,drm`
  features at the 22-warning baseline; `recv_thread` stays as a tracked
  exception. `3cb42f672`.
* `split_audit.py` now checks that a split commit loses no code, and running
  it before landing a split is a rule; the audit of the 09-12/13 splits found
  no functional loss, with the assistance-page split recorded as a reviewed
  exemption. `be8b42a70`, `0fc4c54e2`, `7778ff8f2`, `755175bcf`.
* hbbs UDP RegisterPeer/RegisterPk handling split into transport-free
  helpers, with the refusal paths now covered. `227edaf`, `38673a2`.

## Performance

Measurements and their limits are in `docs/perf-baseline-2026-09-13.md`
(`23fb8ecbd`, `b9aafc583`, `b45b10a3f`, `40d12b808`, `3854ac04c`).

* Standby cost on a controlled machine, from about 217 timer wake-ups a
  second to about 6. Services with no subscribers sleep until one arrives
  rather than polling their own subscriber list thirty times a second
  (`de195fc4d`, 6 unit tests); the child reaper waits to be woken by a spawn
  instead of looking ten times a second at a list that is empty unless a
  session is running (`5ee0db0b5`, 6 unit tests); the Windows service hears
  about a session change instead of enumerating sessions three times a second
  (`94b4a7b84`, 2 unit tests); the heartbeat collects system information only
  when it could upload it, not on every three second tick (`970c15fa6`, 5
  unit tests); the tray asks for the session count every three seconds rather
  than every second (`76ff65ed8`). Real-machine before and after is pending a
  virtual machine that was powered off.
* The single most valuable number of the day: sessions to the LAN peer had
  always gone through the relay at about 1.3 s, because the peer was
  registered by hand and had no inbound firewall rule, so the successful
  punch could not be accepted. With a rule it is 11 to 20 ms, direct. The
  path that found it, worth reusing: the server log shows the rendezvous
  deciding `local_addr`, so the peers are on one segment and the punch
  itself worked; the client then times out after its 1 s direct attempt and
  asks for a relay; which points at the peer accepting nothing inbound, and
  the peer had no rule because it was registered without the installer.
* Hardware encoders now follow the frame rate QoS actually paces the capture
  at, instead of a hardcoded 30. `a6ad433d2`, `cfecf0155`, `593b91cda`,
  `64f9f9ea1`, unit tests plus the `video_qos` simulation.
* The bitrate recovers exponentially after a cut, with the rejected 1.5x
  headroom written down. `6fc67873d`, `c86ed869e`, 73 `video_qos` tests.
* `Auto` skips software AV1 on machines with at most four cores.
  `34ab5730d`, 6 unit tests; confirmed on the two-vCPU peer, which now
  negotiates VP9.
* Frame fetches wait on a plain channel rather than a fresh runtime per
  frame, and the wait is paced by the link with the encode held instead of
  queued. `b56219d25`, `0337e147a`, 9 unit tests. A real-machine before and
  after was run twice and is flat within noise; the reason is structural and
  is recorded in the baseline.
* A bitrate base table matching what screens need, and the client video
  queue holds half a second and shows only the newest decoded frame.
  `cfc4dc9c1`, `32710affb`, `5d6628558`, `9a82cf100`.

## Infrastructure

* **Both repositories are public.** History was rewritten twice with
  `git filter-repo`: first to replace real hosts, keys, account names and
  cloud ids with placeholders, then to strip AI attribution trailers from
  every commit message. Each rewrite was verified tree by tree against the
  pre-rewrite commits, the commit and tag counts were unchanged, and the
  result was force-pushed to GitHub and to the build machine.
  `45e780e4b`, `c4541d285`, `3955428`, `dfd67d1`.
* A `commit-msg` hook rejects any AI attribution trailer, installed from
  `tools/git-hooks/install.ps1` into the shared hooks directory so every
  worktree runs it; it must not be bypassed with `--no-verify`.
  `6401378a9`, `a78b681`.
* GitHub keeps only `master`; all `ci/*` branches were deleted and Linux
  verification moved to the build machine's WSL Debian. `bd8887f18`,
  `3fa0fe646`.
* The Windows build is green on master and its artifact carries the portable
  zip, the MSI and the checksum list. A shared script now asserts all three
  are present and that `sha256sum -c` passes, in both the build and the
  release workflow, because `if-no-files-found` only fires when every glob
  misses; `*.sh` is pinned to LF so the script survives checkout on the
  Windows runner. `98d439287`, `25a1f4974`.
* The MSI was validated on the VM by silent install, twice: a plain install
  and one carrying the configuration environment variable, which imported
  two settings and locked four. The service, its two firewall rules, the
  four server settings and the registration log were all checked, and the
  device id is unchanged. `53d706b2c`. The first round's
  `Failed to start service: OpenUUConfigImport, error: 0x41D` is a designed-in
  expected path, not a fault: `--import-config` exits without reporting to
  the service control manager, so the start request always fails once the
  process has finished (`res/msi/CustomActions/ServiceActions.cpp`, which says
  so in a comment), and `MyStartServiceW` logs every failure with the same
  wording. The paired `Failed to open service ... 0x424` comes from the
  pre-install sweep for a stale temporary service, which a first install has
  no reason to find. The import itself succeeded in both rounds.
* The server was deployed again for the UDP NAT fix, with the usual backup,
  health check and rollback path.
* A restyle plan for the session window was written before the work started.
  `3c3e1649e`.

## Not verified, and why

* **Cross-network smoke.** Both test peers share one public address, so the
  peer-initiated relay path and symmetric-NAT behaviour are still only
  covered by unit tests.
* **Hardware-encoder numbers on a real machine.** They need a peer that has
  a GPU *and* composites a moving screen. The build machine has the GPU but
  is headless, so DXGI produces no frames; the VM composites but encodes in
  software. An HDMI dummy plug on the build machine would settle it. This
  blocks the before and after for the frame-rate lock, the recovery pace and
  D3D decoding on the controller.
* **Capture pacing on a real machine** is flat for the structural reason
  above: the two-vCPU peer's encoder, not the link, is the bottleneck, so
  the wait path never approaches the ceiling the change addresses.
* **A direct session to the VM peer** is impossible: it sits on the Hyper-V
  internal subnet, which the controller cannot route to. Relay only.
* **Remote-session screenshots** for the restyled window are still open.
* **Start-up drain, P0-c and tokenising the file-transfer and terminal
  pages** have not been started.

## Appendix: what went wrong in this session, and what it became

Seven mistakes of my own, each with what it cost and what now prevents it.
They are recorded because five of them turned into rules, which is the only
reason they are worth keeping.

| What I got wrong | How it surfaced | What prevents it now |
| --- | --- | --- |
| Reported a build as finished by reading the last line of its log; it had been killed and its output directory was already empty | The task notification said `killed`, contradicting what I had just said | `build-flutter.ps1` asserts the shipped files exist and were written by that run; AGENTS.md: judge a background task by its exit code or its artifacts |
| Blamed the build for the machine's memory pressure, as did the person who asked | Measured it instead of arguing: ssh and scp held 22 MB, eight editor processes held 3.1 GB | Nothing to fix in the product; the lesson is that "who used the memory" is a measurement, not a deduction |
| Broke the shared build script by piping a Windows path through a heredoc, turning `\build` into a backspace, then overwrote another session's fix of the same line | Another session hit the broken script and repaired it independently | AGENTS.md: transfer a file rather than pipe content, diff the remote copy before overwriting, run it once after installing, say which script changed |
| Wrote my own test-method error into the baseline as if it were a product defect: `codec-preference` had been put in the wrong file | Re-read the option's only call site while auditing something else | The option's real location is documented; the habit is to suspect the method before the product when an experiment fails |
| Stated in a new rule that IPv6 punching only contacts our own server, and defaulted it on accordingly — so my own change broke the rule I had just written | Read `test_ipv6` while writing the explanatory document | The rule now says to check what the code does when nothing is configured, not what the feature's name suggests |
| Called an unreachable path the exposure: the "manual update check" has no callers, while the reachable path was a switch Windows showed on every install | Went to fix it and could not find the button to fix | Judge reachability by walking the whole path from interface to implementation, not from one side's call graph |
| Wrote a test that proved a request is not sent by calling the thing that sends it; under test the app name is still upstream's default, so it would have sent a device fingerprint from the build machine | An unrelated assertion on the same line failed first and stopped it | The rule is asserted as a pure function of the name; to verify that something does not happen, do not execute it |

The fixture that was read as display corruption belongs here too: an
unlabelled test pattern cost another session an interrupt and reached the
user as a suspected bug. Fixtures now carry a banner saying what they are.

## Two commits carry changes that are not theirs

Left in the record deliberately, because history was not rewritten and someone
will eventually run into it.

While repairing compile errors in the connection writer split, repeated
`git commit --amend` in the shared working tree landed on other sessions'
commits: between one amend and the next, another session had committed, so
HEAD was no longer mine.

* `55a37150b`, whose subject is the third-party endpoint audit, also carries 41
  lines of `src/stream_split/{mod,tcp_split,tests}.rs`.
* `2fa5530c1`, whose subject is the lessons page, also carries one line of
  `src/stream_split/tcp_split.rs`.

Both authors' own content is intact: an amend with a pathspec replaces only the
listed paths and keeps the rest. Nothing was lost, and the tree at master is
correct.

**`3841bac47` does not compile on its own.** It is the first commit of the
writer split, and its repairs ended up in the two commits above, so a `git
bisect` that lands on it will fail to build for a reason that has nothing to do
with what is being bisected. Skip it.

The rule that came out of this is in AGENTS.md: in a shared working tree
`--amend` and `rebase` are never used, because HEAD may not be your commit.
