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
