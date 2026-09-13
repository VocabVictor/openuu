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
* **Taking the video write out of the connection's main loop.** A write that
  blocks holds the whole `select!`, so while a frame is going out the loop is
  not reading the socket either: input, clipboard and the probe replies wait
  behind the picture. Measured at up to 1.4 s on a 2 Mbps link. The design
  that fixes it is a writer task with the control messages ahead of the
  video, and it needs the socket split into a reading half and a writing
  half. `Stream` is `hbb_common`'s enum over TCP, WebSocket and WebRTC, and
  its send path carries the encryption sequence, which only one writer may
  own; splitting it is a change to the submodule, not to this repository.
  What was done instead bounds the damage rather than removing it: the
  capture loop holds the encode while a frame is unfetched, the controller
  measures the link from the blocked send and cuts the bitrate to fit, and a
  write that stalls now ends the session in five seconds rather than twelve.
  The remaining coupling is bounded by how long one frame takes to flush.
* **Whether to fork `hbb_common` at all.** The submodule points at
  `rustdesk/hbb_common`, the upstream repository, and the commits in this
  repository that touch it only move the pointer to one of theirs. OpenUU is
  an independent product and is now public, so anything that has to change
  the protocol floor -- the WebSocket transport, KCP, `FramedStream`
  behaviour, the rendezvous messages -- is currently out of reach. The
  trigger for deciding is the first change we genuinely cannot route around.
  On 2026-09-13 two came up in one afternoon, which is why this is now a
  decision rather than a note:

  * the WebSocket Nagle line above, which costs nothing today because nothing
    uses ws, and
  * taking the video write out of the connection loop (the item above it),
    which is the fix for input freezing behind a stalled picture and needs
    `Stream` split into a reading and a writing half. **This entry said that
    one had no workaround at all. That was wrong**; see
    `docs/submodule-decision.md` section 2. The send and receive counters are
    independent fields of one struct, WebRTC already clones, and the TCP
    fields are public, so the split is buildable here for both. Only
    WebSocket is closed, and nothing uses WebSocket by default.

    **Where this actually stands as of 2026-09-13 evening:** `src/stream_split/`
    exists and splits a TCP connection, with five tests. **Nothing calls it.**
    The connection loop still owns one `Stream`, so the coupling is exactly what
    it was and none of the benefit has been collected yet. Still to do: give the
    writer its own task and put control messages ahead of the video (with the
    ordering test that belongs to it), then the WebRTC half.

  What forking would cost, as far as it can be estimated from this side: the
  pointer has moved 19 times in the last 90 days, about twice a week, and the
  moves are whole upstream merges (WebRTC, the base crate split, port forward
  changes), not small patches. Every one of those would become a merge we
  perform instead of a pointer we move, against a library whose internals we
  do not follow day to day. Against that, our divergence today is zero: we
  have changed nothing in it, so a fork starts cheap and gets more expensive
  the longer we hold changes that upstream does not want. The other path,
  sending changes upstream, costs us their schedule and their view of what
  belongs in a shared library; the ws line would plausibly be accepted, the
  socket split much less so, since it changes a type every one of their
  transports goes through.

  Answered on 2026-09-13 in `docs/submodule-decision.md`: do not fork, build
  the writer split here for TCP and WebRTC, and revisit if WebSocket becomes a
  transport people use, if the protocol floor itself has to change, or if
  upstream breaks the wrapper more than about twice.

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
  screen. **Precondition met as of 2026-09-13**: the Android build is green and
  produces an APK with both native libraries in it (`android-build.yml`, run
  34755672904). What is still missing is a device: nobody has installed the
  APK, so this item now needs someone with an Android phone, not more CI.
* **Dark-mode tokens.** Dark is now a supported appearance and the naming and
  resolution mechanism are decided (`docs/dark-token-decision.md`,
  `UiPalette extends ThemeExtension`, `UiColor.of(context)`).
  **Status as of this entry: the decision is written, the implementation is
  not. `UiPalette` does not exist in `ui_tokens.dart` yet, so no file can be
  hooked up.** Once it lands, each file is three steps: take
  `final ui = UiColor.of(context);` at the top of `build`, swap `UiColor.x` for
  `ui.x` (and `UiType.y` for `type.y`), and drop the `const` that no longer
  holds. Files are independent, so they can be migrated one at a time.

  Route map for the session-window and dialog files, counted on the master of
  this entry — 132 token references across 16 files, 11 of them inside a
  `const` expression:

  | refs | in const | file (under `flutter/lib/`) |
  | ---: | ---: | --- |
  | 19 | 1 | `desktop/pages/terminal_sessions_page.dart` |
  | 14 | 0 | `common/widgets/ui_fields.dart` |
  | 14 | 1 | `desktop/pages/port_forward_page/tunnels.dart` |
  | 13 | 0 | `desktop/widgets/session_status_bar.dart` |
  | 12 | 1 | `desktop/widgets/file_transfer_layout.dart` |
  | 12 | 0 | `desktop/pages/file_manager_page/view_head_tools.dart` |
  | 11 | 3 | `common/widgets/ui_dialog.dart` |
  | 8 | 0 | `desktop/widgets/tabbar_widget/tab_item.dart` |
  | 7 | 3 | `desktop/widgets/remote_toolbar/theme.dart` |
  | 5 | 0 | `desktop/widgets/remote_toolbar/monitor_menu.dart` |
  | 5 | 0 | `desktop/widgets/tabbar_widget/action_buttons.dart` |
  | 5 | 1 | `mobile/pages/server_page/connection_manager.dart` |
  | 3 | 0 | `desktop/widgets/remote_toolbar/toolbar_layout.dart` |
  | 2 | 0 | `desktop/widgets/remote_toolbar/small_menus.dart` |
  | 1 | 1 | `desktop/widgets/remote_toolbar/menu_buttons.dart` |
  | 1 | 0 | `desktop/widgets/remote_toolbar/draggable_show_hide.dart` |

  Two things that are not a mechanical swap: `ui_dialog.dart` and
  `ui_fields.dart` hard-code `Colors.white` for button and field faces, which
  becomes `ui.panelBg` rather than anything derived from black — a dark surface
  is not the light one inverted. And `remote_toolbar/theme.dart` still holds
  five literal colours for the dark appearance (the active hover tint, two
  danger tints, the dark icon colour, the dark bar background); those are the
  ones the palette should absorb, not the light values beside them.
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
