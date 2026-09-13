# Split audit, 2026-09-13

Every refactor/move commit since 2026-09-12 in `openuu` and `openuu-server` was checked
for code that a mechanical split should not lose. The check is the script
`split_audit.py` in the toolchain repository (outside this tree); this file records its
results and the verdict on each finding.

## Method

Two comparisons, both on normalised lines (whitespace collapsed, `pub(...)` visibility,
`super::`/`crate::` path prefixes and receiver names such as `conn.`/`this.` removed;
comments, imports, attributes, braces and other trivial lines dropped):

1. **Per commit**: the multiset of lines the commit's parent had in every touched file,
   minus the multiset those files hold after the commit. What remains is what the commit
   removed without re-adding anywhere it touched.
2. **Per chain**: for every `X.rs -> X/...` rename since the start date, the parent's `X.rs`
   against the union of `X/**` at the current `master`. This catches a loss anywhere in a
   multi-commit split, but also reports later intentional edits to the same code.

`Union::Variant(` and `ipc::Data::Variant` patterns are counted separately, so a dropped
match arm is named even when its body lines happen to exist elsewhere.

Limitation: a line that exists twice before and once after is reported, but an arm whose
body is a verbatim copy of another arm in the same file set is not distinguished from it.

## openuu-server

Commit level (5 move commits):

| Commit | Date | Subject | Lines lost | Arms lost |
| --- | --- | --- | --- | --- |
| 227edaf36 | 2026-09-13 | refactor(hbbs): split the UDP RegisterPeer/RegisterPk handling into tr | 14 | 0 |
| 3ea7194cc | 2026-09-13 | refactor: split rendezvous_server.rs into rendezvous_server/ module | 0 | 0 |
| dae1a055f | 2026-09-13 | refactor: split relay_server.rs into relay_server/ module | 0 | 0 |
| 176717ac9 | 2026-09-13 | refactor: split account.rs into account/ module | 0 | 0 |
| 419d02ff0 | 2026-09-13 | refactor: split common.rs into common/ module | 0 | 0 |

Chain level:

| Rename commit | Original file | Files now | Lines not found | Arms not found |
| --- | --- | --- | --- | --- |
| 3ea7194cc | `src/rendezvous_server.rs` | 15 | 25 | 0 |
| dae1a055f | `src/relay_server.rs` | 6 | 3 | 0 |
| 176717ac9 | `src/account.rs` | 10 | 0 | 0 |
| 419d02ff0 | `src/common.rs` | 5 | 0 | 0 |

Verdicts:

* `3ea7194` (rendezvous_server.rs split): 0 lines lost at commit level. The 25 chain-level
  lines are later intentional changes: the `Sink`/`Data::Msg` plumbing replaced by the
  TCP registration work, UDP `RegisterPeer`/`RegisterPk` handling restructured into
  transport-free helpers (`227edaf`, covered by its tests), the TCP `RegisterPk` "not
  supported" arm replaced by real TCP registration, and the `RequestRelay` token now
  swapped for a single-use peer ticket (`relay_forward_tests.rs`).
* `227edaf` (14 lines): the removed `update_addr`/`send_rk_res` call sites are the
  restructure itself; the helpers return the result and one call site sends it.
* `dae1a05` (relay_server.rs split, 3 lines): `redeem_ticket` moved to `relay_server/ticket.rs`
  and account; two "Relay of ... closed" log lines were dropped later. Cosmetic.
* The UDP `TestNatRequest` reply missing from `handle_udp` (fixed in `19bcbd0`) is **not** a
  split loss: the open-source server never had that arm; it only answered on TCP.

## openuu

Commit level, entries with findings only ({len(c_commit)} of 469 audited commits):

| Commit | Date | Subject | Lines lost | Arms lost |
| --- | --- | --- | --- | --- |
| ea649aa10 | 2026-09-13 | refactor(client): move the punch attempts loop out of Client::_start_i | 29 | 0 |
| 9a48f6289 | 2026-09-13 | refactor(client): move the RelayResponse arm out of Client::_start_inn | 9 | 0 |
| 6e29747f9 | 2026-09-13 | refactor(server): move construction and teardown out of Connection::st | 14 | 0 |
| 9ab7ab1bf | 2026-09-13 | refactor(server): move the send and timer arms out of Connection::star | 1 | 0 |
| f624e771f | 2026-09-13 | refactor(client): move the file transfer arms out of Remote::handle_ms | 1 | 0 |
| 08645636e | 2026-09-13 | refactor(client): move the clipboard arms out of Remote::handle_msg_fr | 1 | 0 |
| c5f6e7120 | 2026-09-13 | refactor(client): move the Misc arm out of Remote::handle_msg_from_pee | 1 | 0 |
| 07ee8d7cc | 2026-09-13 | refactor(client): move the session arms out of Remote::handle_msg_from | 6 | 0 |
| bf71595ef | 2026-09-13 | refactor(flutter): move the main-window handlers into desktop_tab_page | 31 | 0 |
| 3a9dd52c8 | 2026-09-13 | refactor(server): move the file-transfer arms out of Connection::on_me | 3 | 0 |
| 33df086f2 | 2026-09-13 | refactor(platform): split the rest of windows.cc into windows_process. | 1 | 0 |
| f21d918fa | 2026-09-13 | refactor(platform): move session queries and the keyboard hook out of  | 1 | 0 |
| a1bee1187 | 2026-09-13 | refactor(server): move the clipboard arms out of Connection::on_messag | 3 | 0 |
| c262406f7 | 2026-09-13 | refactor(server): move the media arms out of Connection::on_message | 4 | 0 |
| c549cdab1 | 2026-09-13 | refactor(flutter): delete web/dummy.dart, moved to native/unsupported_ | 12 | 0 |
| 925425392 | 2026-09-13 | refactor(flutter): extract DeviceCard from the grouped devices list | 8 | 0 |
| 2a15de7a0 | 2026-09-13 | refactor(flutter): split desktop_assistance_page.dart into desktop_ass | 52 | 0 |
| b2c09980f | 2026-09-13 | refactor(server): move the Misc arm out of Connection::on_message | 1 | 0 |
| c2fd49a98 | 2026-09-13 | refactor(server): move the input arms out of Connection::on_message | 4 | 0 |
| d9904e8a0 | 2026-09-13 | refactor(server): move the pre-auth branches out of Connection::on_mes | 1 | 0 |
| a1a145290 | 2026-09-13 | refactor(base): move the option key constants out of config/keys mod.r | 2 | 0 |
| d692c94f0 | 2026-09-13 | refactor(app): move tray.rs to tray/mod.rs | 2 | 0 |
| 78ae0e2a5 | 2026-09-13 | refactor(models): move UserModel login and logout into an extension | 3 | 0 |
| c722c4a21 | 2026-09-13 | refactor(server): remove the video_qos test files superseded by their  | 1475 | 0 |
| 0f9a396d9 | 2026-09-13 | refactor(privacy_mode): split win_topmost_window.rs into a module | 4 | 0 |
| b15c17e4d | 2026-09-13 | refactor(virtual_display_manager): move the amyuni_idd module out of m | 4 | 0 |
| a3e88d0ce | 2026-09-13 | refactor: remove the Sciter UI | 1232 | 1 |
| facd4cb16 | 2026-09-13 | refactor(ipc): move ipc/auth.rs to ipc/auth/mod.rs | 171 | 0 |
| aa8684e30 | 2026-09-13 | refactor(desktop): move the view camera tab menu and window method han | 2 | 0 |
| bd803502a | 2026-09-13 | refactor(models): move ChatModel overlays and message handling into ex | 9 | 0 |
| dc654f35c | 2026-09-13 | refactor(common): move the peers view list builder and online polling  | 1 | 0 |
| 3b8e681a3 | 2026-09-13 | refactor(desktop): move the remote tab menu and window method handler  | 2 | 0 |
| 17c4634c8 | 2026-09-13 | refactor(models): move TerminalModel session control and input sending | 1 | 0 |
| 6eb6c5802 | 2026-09-13 | refactor(models): move TerminalModel response and output handling into | 9 | 0 |
| ce34a1658 | 2026-09-13 | refactor(common): move the raw touch tap and pan handlers into extensi | 28 | 0 |
| 711ee03a2 | 2026-09-13 | refactor(desktop): move the view camera page body and view builders in | 1 | 0 |
| 902cc7efc | 2026-09-13 | refactor(mobile): move the view camera page body builders and action m | 3 | 0 |
| c196f233a | 2026-09-13 | refactor(mobile): move the terminal page body and floating keyboard in | 2 | 0 |
| 939e06962 | 2026-09-13 | refactor(desktop): move terminal tab session and tab management into e | 3 | 0 |
| fb604618a | 2026-09-13 | refactor(mobile): move the floating left/right button out of floating_ | 1 | 0 |
| 4beb23cbb | 2026-09-13 | refactor(models): move ServerModel service control and the Client clas | 7 | 0 |
| 52d841d65 | 2026-09-13 | refactor(models): move ServerModel client handling and dialogs into ex | 9 | 0 |
| 72cf0cf73 | 2026-09-13 | refactor(mobile): move file manager import/export and bottom sheet int | 4 | 0 |
| d6a041c03 | 2026-09-13 | refactor(models): move RelativeMouseModel event handling and native mo | 11 | 0 |
| 3a843cbcb | 2026-09-13 | refactor(models): move RelativeMouseModel mode switching and mouse mov | 6 | 0 |
| 02d198bc9 | 2026-09-13 | refactor(models): move RelativeMouseModel pointer lock and cursor clip | 3 | 0 |
| 28b306ca3 | 2026-09-13 | refactor(desktop): move the home page left pane builders and setPasswo | 1 | 0 |
| 019ccb478 | 2026-09-13 | refactor(mobile): move floating mouse movement and build helpers into  | 2 | 0 |
| 5fa2561ec | 2026-09-13 | refactor(common): move the OIDC login button and WidgetOP out of login | 1 | 0 |
| 3ee17625d | 2026-09-13 | refactor(desktop): move the remote page body builder and small widgets | 1 | 0 |
| 765070515 | 2026-09-13 | refactor(mobile): move remote page keyboard handling and body builders | 8 | 0 |
| e0c305fac | 2026-09-13 | refactor(desktop): move the file manager file list and its event handl | 2 | 0 |
| d7425f8d7 | 2026-09-13 | refactor(platform): move service loop and process launch out of window | 1 | 0 |
| 8cb75f0d3 | 2026-09-13 | refactor(models): move InputModel pointer routing into extensions | 1 | 0 |
| 8b543ffa2 | 2026-09-13 | refactor(models): move InputModel trackpad and touch handling into ext | 8 | 0 |
| 7595e2cf7 | 2026-09-13 | refactor(models): move InputModel key/mouse sending and mouse movement | 3 | 0 |
| 440028505 | 2026-09-13 | refactor: split desktop/pages/desktop_setting_page.dart into desktop_s | 20 | 0 |
| 6a18c4863 | 2026-09-13 | refactor: split src/common.rs into common/ module | 1 | 0 |
| de28de8d1 | 2026-09-13 | refactor: split models/model.dart into models/model/ parts | 9 | 0 |
| 1ddf11034 | 2026-09-13 | refactor: split common.dart into common/ modules | 7 | 0 |
| 6a51278a9 | 2026-09-13 | refactor: split desktop/widgets/remote_toolbar.dart into remote_toolba | 10 | 0 |

Chain level:

| Rename commit | Original file | Files now | Lines not found | Arms not found |
| --- | --- | --- | --- | --- |
| dd41b87c4 | `src/server/rdp_input.rs` | 5 | 0 | 0 |
| 6705b7e3c | `src/server/wayland.rs` | 9 | 0 | 0 |
| dd1128464 | `src/server/uinput.rs` | 9 | 0 | 0 |
| 74b218169 | `src/ipc/fs.rs` | 6 | 0 | 0 |
| d1440d8b6 | `src/server/connection.rs` | 60 | 36 | 0 |
| d692c94f0 | `src/tray.rs` | 2 | 2 | 0 |
| 98d35060f | `libs/base/src/config/keys.rs` | 10 | 2 | 0 |
| 01178226c | `src/kcp_stream.rs` | 3 | 0 | 0 |
| e2eb0cbe4 | `src/quick_launch.rs` | 5 | 1 | 0 |
| 449cce3e2 | `src/lang.rs` | 59 | 0 | 0 |
| ea8ff99c8 | `src/lan.rs` | 3 | 0 | 0 |
| e51dff6f3 | `src/platform/win_device.rs` | 4 | 0 | 0 |
| ba800c034 | `src/clipboard_file.rs` | 3 | 0 | 0 |
| 591e8aaf2 | `src/hbbs_http/downloader.rs` | 3 | 0 | 0 |
| 020b8c596 | `src/hbbs_http/http_client.rs` | 3 | 11 | 0 |
| c64d7bc19 | `src/hbbs_http/account.rs` | 3 | 0 | 0 |
| e7fa8c6b3 | `src/hbbs_http/sync.rs` | 3 | 1 | 0 |
| af4f7b0e0 | `src/privacy_mode/win_virtual_display.rs` | 3 | 0 | 0 |
| eda23aa00 | `src/privacy_mode/win_topmost_window.rs` | 3 | 4 | 0 |
| 924175e76 | `src/privacy_mode.rs` | 13 | 0 | 0 |
| cc2eb3d2a | `src/updater.rs` | 4 | 0 | 0 |
| 5690861e2 | `src/core_main.rs` | 3 | 14 | 0 |
| 254d3ac92 | `src/virtual_display_manager.rs` | 5 | 4 | 0 |
| aec8eff0c | `src/server.rs` | 216 | 1 | 0 |
| 9b026dc41 | `src/clipboard.rs` | 5 | 0 | 0 |
| facd4cb16 | `src/ipc/auth.rs` | 7 | 61 | 0 |
| 71a307a7b | `src/port_forward_mux.rs` | 11 | 0 | 0 |
| ef1b1fc0a | `src/port_forward.rs` | 5 | 0 | 0 |
| 512d2413a | `src/ui_interface.rs` | 10 | 25 | 0 |
| a26a410ff | `src/platform/windows/acl.rs` | 6 | 0 | 0 |
| 09b882c44 | `src/ui_cm_interface.rs` | 9 | 1 | 0 |
| 14d1769b6 | `src/ipc.rs` | 24 | 0 | 0 |
| 1d53dc382 | `libs/scrap/src/common/vram.rs` | 3 | 0 | 0 |
| b049ade4e | `libs/scrap/src/common/hwcodec.rs` | 5 | 2 | 0 |
| cd7328af6 | `libs/portable/src/bin_reader.rs` | 2 | 1 | 0 |
| 48cfb7a39 | `libs/scrap/src/common/aom.rs` | 5 | 0 | 0 |
| 1f3e9cc94 | `libs/scrap/src/common/vpxcodec.rs` | 7 | 0 | 0 |
| fdbc5e83a | `libs/scrap/src/common/record.rs` | 6 | 0 | 0 |
| 677e3876f | `libs/enigo/src/win/win_impl.rs` | 4 | 0 | 0 |
| 086eefc46 | `src/rendezvous_mediator.rs` | 10 | 4 | 0 |
| 136d00471 | `libs/scrap/src/dxgi/mag.rs` | 6 | 0 | 0 |
| 6560f201b | `libs/scrap/src/common/codec.rs` | 7 | 0 | 0 |
| 0b6ff3228 | `libs/clipboard/src/platform/windows.rs` | 9 | 0 | 0 |
| 978b9acfa | `libs/base/src/fs.rs` | 15 | 13 | 0 |
| 1f2a1bd92 | `src/keyboard.rs` | 9 | 6 | 0 |
| 168e99ce7 | `src/server/service.rs` | 4 | 0 | 0 |
| 0a1887441 | `src/server/clipboard_service.rs` | 5 | 0 | 0 |
| 9a1f6767f | `src/server/port_forward_mux.rs` | 6 | 0 | 0 |
| 0dc5c89ed | `src/server/audio_service.rs` | 14 | 0 | 0 |
| 7915d03ec | `src/ui_session_interface.rs` | 12 | 53 | 0 |
| 0f99bbf84 | `src/server/video_qos.rs` | 32 | 0 | 0 |
| f871047b0 | `src/server/display_service.rs` | 8 | 0 | 0 |
| fad1cbdb0 | `src/server/terminal_helper.rs` | 8 | 2 | 0 |
| bc41566fe | `src/flutter.rs` | 14 | 0 | 0 |
| 9789684a2 | `src/server/video_service.rs` | 10 | 0 | 0 |
| 07eb4e2e4 | `src/platform/windows.rs` | 38 | 9 | 0 |
| 07eb4e2e4 | `src/server/portable_service.rs` | 13 | 22 | 0 |
| 3a5429470 | `flutter/lib/models/file_model.dart` | 11 | 38 | 0 |

Verdicts:

* **No functional loss found.** Every non-trivial difference traces to one of:
  * the split itself renaming a receiver or binding (`conn.` -> `self.`, `let mut x` ->
    struct field, `break` -> `return`), which the normaliser does not fully fold
    (`ea649aa`, `9a48f62`, `6e29747`, `9ab7ab1`, `07ee8d7`, the `on_message`/
    `handle_msg_from_peer` arm moves);
  * intentional deletions whose subject says so (`a3e88d0` remove the Sciter UI, the one
    "arm lost" is a Sciter-only `Data` variant; `c722c4a` superseded test files; `c549cdab`
    web/dummy.dart; `bf71595` DesktopHomePage deleted with the handlers moved);
  * Sciter-only `#[cfg(not(feature = "flutter"))]` code removed by `3b0aa5c` "drop the
    Sciter-only cfg branches" (`facd4cb` ipc/auth: the portable-helper hash trust check and
    its test lived only in that branch; `7915d03` ui_session_interface, `512d241`
    ui_interface, `07eb4e2` portable_service logon helper);
  * later feature or cleanup commits on the same files (`d1440d8` connection.rs: account
    login checks; `978b9ac` fs.rs: `send_current_digest` was dead code removed by `b7a40ba`;
    `020b8c5` http_client TLS-type cache reshaped; `3a54294` file_model.dart web-only paths;
    `5690861` core_main import_config restructure);
  * Dart `this.`/`widget.` receivers and UI restyles inside split commits (`2a15de7`
    desktop_assistance_page: 52 lines of the old card layout; `ce34a16` touch handlers).
* Two chains were not bisect-clean: `facd4cb` (ipc/auth) removed 399 lines in the rename
  commit that the next commit re-added under `windows_*.rs`; `ipcdrm` step 2 had a
  `pub(super)` tuple field the Linux check caught before landing. Both end states are
  complete.

## Follow-ups

* `2a15de7` (desktop_assistance_page): reviewed with the author; the 52 lines are dartfmt
  re-wrapping and the private renames an extension split needs (`caption` -> `_caption`,
  `setState` forwarded). No line or style was dropped. Known exemption, nothing to fix.
* Run `split_audit.py` on the next batch of splits before landing; a per-commit result of
  0 lines and 0 arms is the expected outcome of a mechanical move.
