# Connection manager restyle plan

The connection manager is the window that opens **on the controlled side**
when someone asks to control this device: the request banner, the accept and
reject buttons, the per-session permission switches and the file-transfer
log. It is still upstream RustDesk's visual design while the rest of the
desktop has moved onto the design tokens, and it is the one surface a
controlled user sees at all, so it is also the surface where our styling
matters most.

Scope: appearance only. No change to what the buttons do, to the permission
model, or to the IPC behind them.

## Where it lives

`flutter/lib/desktop/pages/server_page/`, 1478 lines in seven files:
`server_page.dart` (124), `connection_manager.dart` (270), `cm_header.dart`
(229), `cm_control_panel.dart` (207), `cm_control_panel_authorized.dart`
(178), `privilege_board.dart` (251), `file_transfer_log.dart` (219). All are
already under the 300-line rule, so no split is needed first.

## What is upstream-styled today

* `cm_header.dart` paints a cyan-to-blue gradient (`0xff00bfe1` to
  `0xff0071ff`) with white text at hardcoded sizes 20, 14 and 12, a 30 px
  app icon and hand-written margins.
* `cm_control_panel.dart` builds accept on `Colors.green[700]` and reject as
  transparent with a `Colors.grey` border, both with `Colors.white` text.
* `privilege_board.dart` and `file_transfer_log.dart` use ambient theme
  colours and their own spacing.
* Nothing in the directory references `UiColor`, `UiType`, `UiSpace` or
  `UiSession`.

## Target

Reuse the existing tokens rather than invent a palette: `UiColor.primary`
for the affirmative action, `UiColor.border` / `UiColor.text` /
`UiColor.textSecondary` / `UiColor.muted` for surfaces and text,
`UiType.sectionTitle` / `rowTitle` / `caption` / `button` for type, and
`UiSpace` for spacing. Add one `UiCm` group to
`flutter/lib/desktop/widgets/ui_tokens.dart` for the few sizes that are
specific to this window (banner height, avatar size, control-bar height,
permission-row height), following how `UiSession` is organised: sizes there,
colours and type from the shared groups.

The banner loses the gradient and becomes a plain surface carrying the
peer's name, id and, when present, the requesting account, in the same type
scale as the rest of the app.

## One constraint that is not cosmetic

This window is a trust decision. The restyle must keep **accept and reject
visually distinct and equally reachable**, and must not make accept the
quiet default that a user clicks past. Concretely: reject keeps a real
border and full-size hit area, accept does not grow relative to it, and
neither is reduced to a bare text link. Any change to the wording of the
request line needs the same care, since that line is what the user judges.
If a proposed visual makes the two harder to tell apart, the visual loses.

## Sequence

One logical unit per commit, each passing `flutter analyze` with no new
diagnostics:

1. `UiCm` token group, no call sites yet.
2. `cm_header.dart` onto the tokens (the banner, the biggest visual change).
3. `cm_control_panel.dart` and `cm_control_panel_authorized.dart` onto the
   tokens, keeping the accept/reject distinction above.
4. `privilege_board.dart` permission rows onto the row/switch tokens already
   used by the settings pages.
5. `file_transfer_log.dart` and the `connection_manager.dart` tab strip.

## Verification

`flutter analyze` against the current baseline on every step, and the peer
side of a real session for the visual check: connect to a test peer, let the
request appear, and capture one screenshot of the banner plus one of the
permission board for a human to judge. That is the only way to see this
window, since it never opens on the controlling side; it needs a peer whose
session is being watched, which the build machine cannot provide headless,
so the VM peer with an interactive session is the one to use.

## Not in scope

The mobile connection manager (`flutter/lib/mobile/pages/server_page/`)
shares the model but not the widgets; it is a separate piece of work.
