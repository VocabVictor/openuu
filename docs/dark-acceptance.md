# Dark appearance: what counts as accepted

The palette and the file-by-file migration are described in
`docs/dark-token-decision.md`. This is the list the work is checked against
when the three sessions have finished, so acceptance is a walk through
checkable items rather than a look at one screenshot.

Every item below is either a command whose output is the verdict, or a named
control on a named screen with a stated expectation. "It looks right" is not
an item.

## 0. The baseline the checks are read against

Measured on the tree these items were written for, and the number each item
is compared with afterwards:

* `flutter analyze` — **222 issues, zero errors.** The count is the detector:
  any item below that leaves it at 222 changed nothing, and one that raises it
  has to say which line it added.
* `ftest.ps1 <branch>` with no path — **160 tests, all passing.** The full
  suite, not a selected file. It is also run by `linux-check.yml` on every
  push to master, which is the correction to a claim made earlier in this
  file's history: the suite was not unrun. It ran every time, it was red, and
  nobody opened the result. A check whose output nobody reads is weaker than
  no check, because it is counted as coverage.

Both are taken **on the build machine**, after `check.ps1 <branch> -Flutter`,
which is what moves that worktree to the commit being measured. **222 is a
build-machine number. The shared working tree here holds untracked generated
files, reads differently, and is not to be used for acceptance.**

Not in the shared working tree on the development machine, where the same
commit measures 228. The six extra are calls to `mainImportConfigText` and
its two neighbours, undefined because `flutter/lib/generated_bridge.dart` is
generated, git-ignored, and stale there. `flutter analyze` reads a working
directory, not a commit, and that directory holds files no commit contains.

The general form is worth more than the instance: **when two machines
disagree about the same commit, the difference is something outside version
control** — a generated file, a cache, an environment variable, a local
config. Chasing it in the diff finds nothing, because it is not in the diff.

## 1. Nothing is left outside the palette

**These commands read more than this round touched.** They cover all of
`flutter/lib/desktop` and `flutter/lib/common/widgets`, while the round
covered the home, the settings area, the session window, the connection
manager and the assistance page. That is deliberate: a command scoped to the
round could only confirm what the round already claims. It is also why the
output has to be read against the exclusions below rather than taken as a
list of defects — a first run prints around ninety-five lines for the first
command alone.

**Do not narrow the commands to make the output smaller.** Adapting a
measurement to its result destroys the only thing it was for. Exclusions are
listed here, where a reader can tick them off one at a time and see what was
left out and why.

Three commands. Each should print nothing, and each line it does print is a
place the appearance cannot reach.

```
# a literal white or black anywhere a surface, a label or an icon is drawn
grep -rn "Colors\.white\|Colors\.black\|Color(0xff\?ffffff)\|Color(0xff000000)" flutter/lib/desktop flutter/lib/common/widgets | grep -v "_test\|ui_palette.dart"

# a token read as a constant in a widget that has a context to read from
grep -rn "UiColor\.[a-z]" flutter/lib | grep -v "UiColor\.of(" | grep -v "ui_palette.dart\|ui_tokens.dart"

# a colour written as a literal in a file that already uses the tokens
grep -rln "UiColor\.of(" flutter/lib | xargs grep -n "Color(0x" | grep -v ui_palette.dart
```

**Read the second command carefully: it is what separates the two rounds.**
The token round replaced scattered literals with `UiColor.<name>` **constants**,
which are the light values by design; the palette round replaced those constants
with `UiColor.of(context).<name>`. A file finished by the first round is full of
`UiColor` and reads as migrated, so grepping for `UiColor` alone says everything
is done. `UiColor\.[a-z]` *without* `UiColor\.of(` is the grep that tells one
round from the other -- and the commit subjects do not, because the first round's
say `... on the design tokens`. Nine files were misread exactly this way on
2026-09-13 (backlog: the files that are on the tokens but not on the palette).

Known and accepted exceptions, which those commands will print:

* `DesktopWelcomePage.blue` is a public `static const`, used by callers inside
  `const` expressions. It keeps the light primary and its doc comment says a
  themed caller reads `UiColor.of(context).primary` instead.
* `UiColor`'s own members and `UiType`'s static styles are the light values on
  purpose: they are the fallback for a widget with no context.
* `flutter/lib/mobile/` is outside this round (backlog: mobile has no tokens
  at all).
* **The QR code in `two_factor_dialogs.dart`.** `QrImageView` is given a white
  background because the white is part of the encoding: many readers refuse an
  inverted code, so a dark-mode QR would simply fail to scan. **Never
  retired** — it is not a surface colour.
* **Everything drawn on top of the preview in `desktop_preview/panel.dart`**
  (a scrim, a caption, a timestamp badge, a reload icon: seven literals). The
  layer underneath is somebody's screenshot, not a themed surface, so the
  scrim darkens a photograph and the white on it means "on a dark scrim", not
  "foreground on the primary colour". Following the theme would put white text
  on a pale screenshot. **Never retired.**
* ~~**The value painted inside the slider thumb in `menu_buttons.dart`.**~~
  **Retired, `440faba31`.** Its release condition was that the painter take the
  colour as a constructor argument, and that is what it now does: both call
  sites resolve `onPrimary` and pass it in. Kept here as a worked example --
  an exception whose reason was *where the code sits* rather than *what it
  draws* is one somebody can end, and this one lasted a few hours.

These three were not one exception. The first two say the colour is not a
surface colour at all; the third said it is, and could not reach the palette
from where it stood. Only the third had a release condition, and only the
third is gone — which is the whole point of writing the condition down.

**Files the palette round never touched.** These last changed in the earlier
light-token round; the palette round did not open them, so their literals are
not regressions of this work and not accepted by this list either. They are
their own backlog entry:

* all of `desktop/pages/server_page/` (the connection manager's control
  panels, header and file-transfer log)
* `desktop/widgets/tabbar_widget/tabbar_theme.dart`
* `common/widgets/overlay/chat_window.dart`, `common/widgets/chat_page.dart`
* `common/widgets/login/login_dialog.dart`, `common/widgets/login/oidc.dart`
* `common/widgets/address_book/tag_widget.dart`,
  `common/widgets/my_group/my_group_state.dart`
* `common/widgets/dialog/two_factor_dialogs.dart`,
  `desktop/widgets/button.dart`, `desktop/widgets/account_action.dart`

**Literals that are not surfaces**, and stay whatever the appearance:

Each carries the condition that would end the exception, so none of them
becomes decoration:

* a QR code's quiet zone (`network_provision.dart`) — part of what a scanner
  reads, not a themed surface. **Never released**: it is a property of the
  format, so no change to the palette can reach it.
* a scrim (`page_tabs.dart`, `desktop_preview`'s overlay) — it darkens what
  is under it rather than being a surface of its own. **Never released** for
  the same reason: its job is defined against whatever is below it, not
  against the page.
* an illustration's own palette (`desktop_welcome_page/devices_painter.dart`)
  — art with no context to read a theme from. **Released if** the painter is
  ever given a palette to draw with, which would be a redesign of the
  illustration rather than a migration of it.

Anything the commands print that is not on one of these lists, or in the
three known exceptions above, is a finding.

## 2. Contrast has a number, not an opinion

The standard is WCAG 2.1 AA: **4.5:1** for text below 18.66px bold / 24px
regular, **3:1** for larger text, icons and the boundary of a control the user
has to find. Measured on the palette as it stands (dark first, light second):

| pair | dark | light |
| --- | ---: | ---: |
| `text` on `surface` | 12.14 | 15.78 |
| `text` on `panelBg` | 13.26 | 15.23 |
| `textSecondary` on `surface` | 9.49 | 7.10 |
| `muted` on `surface` | 5.11 | 3.25 |
| `faint` on `surface` | 3.04 | 1.99 |
| `primary` as text on `surface` | 4.37 | 4.59 |
| `danger` on `surface` | 5.16 | 3.71 |
| `ready` on `surface` | 8.48 | 2.28 |
| `onPrimary` on `primaryFill` | 4.93 | 4.59 |

Two things to read out of that table rather than past it:

* **Dark is not the weaker appearance.** Six of the nine pairs score higher in
  dark than in light. `faint` on the light surface is 1.99 and `ready` on it is
  2.28 — both below 3:1, both pre-existing, neither introduced by this work.
  They belong in the light-appearance backlog, not in this acceptance.
  **Dark is never to be pulled down to light's level for the sake of
  consistency.** Where the two differ and dark scores higher, light is the one
  with the defect; reading the difference as a dark-appearance fault and
  "fixing" it makes the better side worse.
* **`primary` and `primaryFill` are two members for one hue on purpose.** White
  on the dark `primary` measures 3.47:1, under the 4.5 a 13px button label
  needs; a blue light enough to read as text on the dark surface is too light
  to carry white text. Any filled control whose content is `onPrimary` uses
  `primaryFill`.

Re-measure after any palette change with the ratio helper in this file's
history, or any WCAG contrast calculator; a pair that drops below its
threshold is a finding, not a matter of taste.

## 3. The two hard rules, control by control

From AGENTS.md. Both are about telling states apart, and both fail silently
when a background moves, so each is checked on a named control.

**A state is never told apart by colour alone.**

| screen | control | second cue that must survive |
| --- | --- | --- |
| device list | presence dot | shape: filled circle / ring / dash |
| connection manager | permission tile | fill against outline |
| settings | switch | knob position, not only track colour |
| session toolbar | active button | the tint plus the pressed shape |

Check by looking at the dark screenshot with the colour removed (any
greyscale filter): each state must still be readable.

**A disabled control must not look like a negative one.** In dark,
`primaryDisabled` (a dimmed blue) and `settingsSwitchOff` (a grey) separate at
1.80:1; in light they separate at 1.03:1, which is to say they are nearly the
same colour there. **Dark is the better of the two and must not be "fixed"
towards light.** Named check: on the settings page in dark, a switch that is off
and a button that is locked must not read as the same treatment.

## 4. Where it most likely breaks

Ordered by how quietly they fail.

1. **A fill that kept `primary`.** White content on it is 3.47:1. Known sites
   still to convert at the time of writing: `port_forward_page/tunnels.dart`
   (two), `widgets/file_transfer_layout.dart`, and
   `mobile/pages/server_page/connection_manager.dart`.
2. **`iconColor` in `remote_toolbar/theme.dart` reverses a colour into a
   meaning** (`if (background == activeColor) ...`). It still works, because
   the values come from one palette instance, but it selects a branch by
   equality of colour: if two members ever hold the same value in one
   appearance, it silently picks the wrong branch. Backlog item; in this
   acceptance, check the toolbar's active and danger buttons in dark by eye.
3. **A widget that renders before the theme is in force** falls back to the
   light constants and looks correct in light, wrong in dark. The tell is a
   single light-coloured control on an otherwise dark screen.
4. **`statusRunningBg` / `statusStoppedBg`** are tinted backgrounds behind
   text; they are the pair most likely to have been mapped by inverting.

## 5. The screenshots, and what each one proves

Taken in dark with `--page`, one per claim, after all three sessions report
their migration finished. A screenshot that proves nothing is not on the list.

| # | screen | proves |
| --- | --- | --- |
| 1 | home, device list | the shell, sidebar, cards and the three presence states in one frame |
| 2 | settings, safety tab | switches, locked controls and the disabled-vs-off distinction |
| 3 | settings, network tab with a dialog open | dialog surface, fields and the three button kinds against a dark page |
| 4 | image 1 again, in greyscale | that every state in it survives without colour |

Three are captures; the fourth is the first one put through a greyscale
filter, which is what makes rule 3 checkable rather than asserted. All four
are taken with `--page`, which needs no click.

### Two claims this list does not cover

The session toolbar and the connection manager cannot be captured this way:
the toolbar's buttons are behind a submenu that has to be opened, and the
connection manager only exists while someone is connected. Verification here
is command-line only and does not automate interaction (AGENTS.md), so these
two are **not verified**, and the report says so rather than implying the
appearance was checked:

* **The toolbar strip, its active and danger buttons, and the quality monitor
  in dark.** What is missing: a person opening the toolbar's Control Actions
  menu once, in dark, and looking at it.
* **The connection manager's permission tiles in dark**, where a granted
  permission is a filled tile and a withheld one an outlined tile. What is
  missing: a person accepting one incoming session in dark.

Neither needs a build, a machine or a plan — only a user who is connecting
anyway. Until then the two claims stay open; they are not to be closed by
inference from the other screenshots, because both are exactly the surfaces
where a tint that was mapped by inverting would show.

## Result of the run on 2026-09-14

**Not accepted.** Two findings, both in the settings window, and both of the
same shape as the home shell defect this list caught the first time.

Scope of this run: **sections 0, 1 and 2 only** — everything the command line
can reach. The screenshots are not part of it, so nothing here says the
appearance was looked at. A list that reads "accepted" while half of it was
never run is worse than one that reads "not accepted".

**Section 0, baseline** (build machine, worktree self-reported as
`68da7776e9`): `fanalyze.ps1` **218 issues, zero errors**;
`ftest.ps1` with no path, **160 tests, all passing**.

**Section 1, the three greps, run verbatim.**

* **Grep 2 (a token read as a constant): 1 hit**, the documented
  `DesktopWelcomePage.blue`. Clean.
* **Grep 3 (a literal in a file already on the palette): 6 hits.** Five are
  the documented ones — the device page's row waiting on the shell, two in
  `tunnels.dart`, one manual dark branch in `file_transfer_layout.dart`, one
  tab hover in `tab_item.dart`. The sixth, `chat_window.dart`'s
  `_remoteChromeFill`, is new: that file has just joined the tokens.
* **Grep 1 (a literal white or black): 38 hits.** Most fall under the
  exclusions above, and one group needs a name the list did not have:
  **content on a veil** — `desktop_preview/panel.dart` (7),
  `quality_monitor.dart` (1) and the scrims already listed. White on a
  black scrim is not a themed surface, same reasoning as the QR quiet zone.
  **Two hits are findings**, both mine:

  **The settings window pins its own light appearance.**
  `desktop_setting_page.dart` sets `scaffoldBackgroundColor` to `0xfff8fbfd`
  and `CardTheme.color` to `Colors.white` for the whole window, so in dark
  the settings page keeps a light ground and white cards whatever the palette
  says. It is the home shell defect again, in the one place a user spends the
  longest. **What is missing: the two values take `panelBg` and `surface`.**

  Three hits in `view_file_list.dart` and one in `menu_buttons.dart` are in
  the session window, which belongs to another session; they are listed here
  rather than fixed.

**Section 2, contrast: 30 pairs, the whole table, zero dark failures.**
`primary` on `surfaceHover` measures 4.46:1 against a 4.5 threshold, and
**that pair does not occur**: the only two places that paint `surfaceHover`
are a device row's hover and the tool tiles in `device_action_bar.dart`, and
neither draws `primary` on it. It is recorded so the next reader does not
repeat the search. `overlayHover` has no ratio: it is translucent by
definition, and what it is measured against is whatever lies beneath.

**Still unverified, unchanged:** the toolbar's buttons in dark and the
connection manager's tiles in dark. Both need a person to open a menu or
accept a session; neither is closed by inference from the frames that were
captured.
