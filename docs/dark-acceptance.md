# Dark appearance: what counts as accepted

The palette and the file-by-file migration are described in
`docs/dark-token-decision.md`. This is the list the work is checked against
when the three sessions have finished, so acceptance is a walk through
checkable items rather than a look at one screenshot.

Every item below is either a command whose output is the verdict, or a named
control on a named screen with a stated expectation. "It looks right" is not
an item.

## 1. Nothing is left outside the palette

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

Known and accepted exceptions, which those commands will print:

* `DesktopWelcomePage.blue` is a public `static const`, used by callers inside
  `const` expressions. It keeps the light primary and its doc comment says a
  themed caller reads `UiColor.of(context).primary` instead.
* `UiColor`'s own members and `UiType`'s static styles are the light values on
  purpose: they are the fallback for a widget with no context.
* `flutter/lib/mobile/` is outside this round (backlog: mobile has no tokens
  at all).

Anything else the commands print is a finding.

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
same colour there. Dark is the better of the two and must not be "fixed"
towards light. Named check: on the settings page in dark, a switch that is off
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
| 4 | a session window, toolbar expanded | toolbar strip, active and danger buttons, and the quality monitor |
| 5 | connection manager | the consent surface: permission tiles filled against outlined |
| 6 | image 1 again, in greyscale | that every state in it survives without colour |

Five are captures; the sixth is the first one put through a greyscale filter,
which is what makes rule 3 checkable rather than asserted.
