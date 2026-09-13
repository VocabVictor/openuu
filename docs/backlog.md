# Backlog

Items ruled "not this round" by the coordinator; each one is a separate task
with its own tests.

* **`Connection` test constructor.** `src/server/connection/conn_struct.rs`
  has no `cfg(test)` constructor, so `Connection::on_message` cannot be
  driven from a unit test; the tests under `src/server/connection/test/` are
  all pure functions. Wanted first for "a LoginRequest on a controlled peer
  without an account session goes straight to scope check and password
  validation" (a84c5c4d1 removed the gate; coverage today is the CVM smoke
  test with the controlled peers signed out). Estimate: half a day.
* **Encrypt the controlled peer's registration channel.** The peer ticket
  hbbs forwards (openuu-server `docs/relay-ticket-peer.md`,
  `docs/relay-ticket-peer-initiated.md`) travels on plain UDP/TCP to the
  peer; the UDP path has no key exchange.
* **Reduce the tracked single-function exceptions** listed in `AGENTS.md`
  (`Connection::start`, `on_message`, `send_logon_response_and_keep_alive`,
  `core_main`, ...); each needs tests before it is broken up.
* **Remote-session toolbar in the new visual style.** The remote / file /
  terminal windows still use the upstream `remote_toolbar.dart`; the new
  shell reaches them through the real session plumbing (`connectInPeerTab`
  -> `connect()`), so the toolbar is a separate rework.
* **Login dialog in the new visual style.** `_openLoginDialog`
  (`common/widgets/login/login_dialog.dart`) is the only sign-in path and
  is reached from the welcome page, the title-bar account menu, the devices
  page and settings; restyle rather than replace.
* **Bundle-swap helper scripts must verify the restore.** A throw-away swap
  script derived its backup directory name from `%time%`, which parsed to an
  empty string, so every swap reused one directory and two rollbacks silently
  restored nothing; the backups were then deleted and the peer's original
  bundle was lost. Any such script names the backup with
  `Get-Date -Format yyyyMMdd-HHmmss` and asserts the restore with `Test-Path`
  (and a file-hash or timestamp check) before deleting a backup.
* **The MSI logs two expected failures as plain failures.** The one-shot
  configuration import service never reports to the service control manager,
  so its start request is meant to fail, and the pre-install sweep for a
  stale temporary service is meant to find nothing; both go through
  `MyStartServiceW` / `OpenServiceW` and are logged as
  `Failed to start service ... 0x41D` and `Failed to open service ... 0x424`,
  which reads like a defect during installer review. Reword them on the
  import path the next time the WiX custom actions are touched, rather than
  rebuilding and revalidating the MSI for a log string.


## The hbb_common submodule, and what it blocks

* **Nagle on the WebSocket transport.** `libs/hbb_common/src/websocket.rs`
  passes a hardcoded `disable_nagle = false` when connecting, so a ws
  deployment pays Nagle against the peer's delayed acknowledgement on traffic
  that is all small messages wanted at once. The TCP transport already
  disables it. Not done because nothing uses ws by default, so the change is
  worth nothing today, and the file is in a submodule we cannot push to (see
  below). Precondition: a deployment that actually needs ws for traversal.
  Two ways in when that day comes: fork the submodule, or send the one-line
  change upstream. A third, worse way exists if only our side may change:
  `WsFramedStream` exposes no accessor for the inner `TcpStream`, so setting
  the option after connecting would need an accessor added there anyway.
* **Whether to fork `hbb_common` at all.** The submodule points at
  `rustdesk/hbb_common`, the upstream repository, and the commits in this
  repository that touch it only move the pointer to one of theirs. OpenUU is
  an independent product and is now public, so anything that has to change
  the protocol floor -- the WebSocket transport, KCP, `FramedStream`
  behaviour, the rendezvous messages -- is currently out of reach. The
  trigger for deciding is the first change we genuinely cannot route around;
  the ws item above is not it, because waiting costs nothing. The two paths:
  fork it and carry the cost of tracking upstream ourselves, or send changes
  upstream and live with their schedule and their view of what belongs in a
  shared library. Nothing to do now: the cost of forking is continuous and
  the cost of waiting is, so far, zero.

## Session-window restyle follow-ups

Left behind by the nine-step restyle (`docs/session-window-restyle.md`). None
of them blocks anything; each says what has to be true before it is worth
doing and what it can break.

* **Dark-mode tokens are missing.** `UiColor` only defines the light palette,
  so the parts of the session window that have to look right in dark mode
  still carry literal values: `_ToolbarTheme` keeps five (the active hover
  tint, the two danger tints, the dark icon colour and the dark bar
  background). Precondition: a decision on whether dark mode is a supported
  appearance at all — if it is, the fix is one `UiColorDark` group plus a
  resolver, not scattered `Theme.of(context).brightness` checks. Impact: every
  file that already uses `UiColor`, because the resolver changes how a colour
  is read; do it in one commit per surface, not repo-wide.
* **The insecure-connection dialog is the last one not on `UiDialog`.** It
  still uses `dialogButton` in `common/msgbox.dart`. Its safe default is
  already correct today (Continue is the outlined secondary, Disconnect the
  filled primary), so this is consistency work, not a fix. Precondition:
  agreement that the mobile shell can take the same visual, since
  `msgbox.dart` serves both. When it moves, **verify the Escape behaviour and
  the button order explicitly**: this is a trust decision, Escape must not
  reach "continue", and the permissive choice must not become the primary.
* **`MenuButton` is shared with the connection-manager window.** The file
  manager head tools were restyled at the call sites only
  (`cd9927603`) because `desktop/widgets/menu_button.dart` is also used by
  `cm_control_panel_authorized.dart`, the toolbar menus and the mod popup
  menu. Precondition: the CM restyle (`docs/cm-restyle-plan.md`) landing
  first, so both windows can agree on one button. Impact if changed early:
  the CM window's accept/reject controls inherit a hover and radius chosen
  for a file toolbar, which the CM plan explicitly protects.

* **Clear the test residue on the Hyper-V peer at its next boot.** The VM was
  shut down on 2026-09-13 to free memory while a measurement fixture was still
  installed, so none of the following could be removed remotely: a local
  administrator account `openuutest`, automatic logon enabled for it with the
  password stored in clear text under the Winlogon key (`AutoAdminLogon`,
  `DefaultUserName`, `DefaultPassword`), an inbound firewall rule
  `OpenUU-perf`, the machine variable `RUSTDESK_QOS_VERBOSE=1`, and helper
  scripts under the shared Public folder. The machine logs into that
  administrator account automatically on boot and the stored password is
  readable by anyone who can read that registry key, so clear this before the
  VM is used for anything else.

## Mobile follow-ups (2026-09 stocktake)

From `docs/mobile-status-2026-09.md`, in the order the coordinator ranked them.

* **Visual confirmation of three dialog regressions.** `98b6185bf` (the 2FA OK
  button never enabling), `ae9482116` (a fixed 352 minimum width overflowing a
  phone) and `741ca0d9b` (the action row not stacking when it does not fit)
  were each verified only by `flutter analyze`. They need one look on a real
  screen. Precondition: the Android build works again
  (`docs/android-build-restore-plan.md`).
* **Dark-mode tokens** — see the entry above; mobile ships a live dark theme,
  so the light-only `UiColor` hurts there first.
* **Mobile has no design tokens at all.** Zero files under `flutter/lib/mobile/`
  reference `UiColor`/`UiSpace`/`UiType` (the connection manager's trust
  controls, `8ae053ef2`, are the first and only exception). Bringing mobile
  onto the shared language is comparable in size to the three desktop rounds.
  Precondition: mobile builds, otherwise the work cannot be seen.
* **Two tidy-ups with no precondition**: fourteen files under
  `flutter/lib/common/` import
  `'../../consts.dart'` with one `..` too many (it resolves — a package URI
  absorbs the extra segment — so this is style, not breakage); and the config
  preview/confirm flow is implemented twice, in
  `common/config_import.dart` for mobile and in
  `desktop_setting_page/network_provision.dart` for the desktop, over the same
  pair of FFI calls.
