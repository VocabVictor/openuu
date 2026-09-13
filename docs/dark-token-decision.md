# Dark appearance: how the tokens resolve

Dark is a supported appearance, not a leftover: Windows 11 users follow the
system theme and the app already carries `ThemeMode` plumbing
(`MyTheme.changeDarkMode`, `lightTheme` / `darkTheme`). What it does not carry
is the design token layer: `UiColor` is a class of `static const Color` and
`UiType` bakes those colours into static `TextStyle`s, so both are fixed to
the light appearance at compile time.

This note fixes the naming and the resolution mechanism, so the files owned by
different sessions can be hooked up independently and identically. Palette
values and the migration itself come after.

## The mechanism: a ThemeExtension, the one already in use

The app resolves its legacy colours through `ColorThemeExtension` and
`MyTheme.color(context)` (`flutter/lib/common/color_theme.dart`). The design
tokens use the same mechanism rather than inventing a second one:

* `UiPalette extends ThemeExtension<UiPalette>` holds one non-nullable `Color`
  per member `UiColor` has today, under the same names.
* `UiPalette.light` and `UiPalette.dark` are registered in the `extensions:`
  list of `MyTheme.lightTheme` and `MyTheme.darkTheme`, beside
  `ColorThemeExtension`.
* `UiColor.of(context)` returns the palette of the appearance in force.
* `UiType.of(context)` returns the same text styles with their colours taken
  from that palette, so a file never pairs a themed colour with a fixed one.

Why not a mutable static swapped on theme change: a widget that does not
rebuild keeps the old colour, and there is no signal to find those widgets.
Why not `if (dark)` at each site: the count is the argument, 227 colour
references in 43 files and 149 type references; a conditional at each of them
is 376 chances to miss one.

`UiColor`'s existing `static const` members stay exactly as they are and keep
their current light values. They are the fallback for a widget with no
context and, while the migration runs, every unmigrated file keeps compiling
and keeps looking the way it looks now. Nothing is migrated by being renamed.

## Hooking one file up

Before:

```dart
Widget build(BuildContext context) => Container(
      decoration: const BoxDecoration(
        color: UiColor.panelBg,
        border: Border(bottom: BorderSide(color: UiColor.border)),
      ),
      child: Text(caption, style: UiType.rowTitle),
    );
```

After:

```dart
Widget build(BuildContext context) {
  final ui = UiColor.of(context);
  final type = UiType.of(context);
  return Container(
    decoration: BoxDecoration(
      color: ui.panelBg,
      border: Border(bottom: BorderSide(color: ui.border)),
    ),
    child: Text(caption, style: type.rowTitle),
  );
}
```

Three mechanical steps, and nothing else in the file changes:

1. `final ui = UiColor.of(context);` at the top of `build`, plus
   `final type = UiType.of(context);` when the file uses `UiType`.
2. `UiColor.x` becomes `ui.x`, `UiType.y` becomes `type.y`.
3. Drop the `const` that the value is now inside. Only 30 of the 227
   references sit in a `const` expression, and the compiler names every one
   — but it names the constructor, not the colour, and a line can hold both
   a palette colour and a `static const` size, so the error reads as though
   the whole widget were at fault. `grep -n "const .*ui\."` over the file
   after the swap finds them faster than one analyze run per mistake.

A file with no `BuildContext` where the colour is needed (a top-level helper,
a `static` builder) takes the context as a parameter rather than reaching for
a global; if that is not practical, it keeps the `UiColor.x` constant and is
listed as unmigrated rather than being half-converted.

## Where it is now

`UiPalette` exists (`flutter/lib/desktop/widgets/ui_palette.dart`), is
registered in both themes, and `UiColor.of` / `UiType.of` resolve it.
`flutter/lib/desktop/widgets/device_row.dart` is migrated and is the worked
example to copy: the palette taken once at the top of each `build`, the same
names read from it, two literal `Colors.white` replaced by `surface` (a card
face) and `onPrimary` (a label on the primary fill).

## When one token is asked for two opposite things

A token that two usages want to pull in opposite directions is not a value
that was chosen badly; it is two responsibilities in one name. Split it, and
leave the existing name to whichever usage already reads correctly.

The worked example is the primary blue. On the dark surface it is read as
text and an icon, which wants it light; underneath white content it is a fill,
which wants it dark. Meeting in the middle fails both — white on it measures
3.47:1 against the 4.5 a 13px label needs, while the same value read as text
drifts down towards the surface. So `primaryFill` is its own member, dark
where it has to be, and `primary` keeps the value it had. In the light
appearance both are the same colour and not a line changes.

The tell is an argument about what a value "should" be where both sides have a
real reason. That argument has no answer as long as the two usages share a
name.

Nothing about this is particular to colour. A timeout that one caller needs
short and another needs long, a config key that means two things to two
subsystems, a function whose callers want opposite defaults: the same
argument, the same answer. Whenever tuning a single value has a real case on
both sides, the name is carrying two responsibilities, and no number settles
it.

## What the palette must satisfy

The dark values are not the light ones inverted. Two rules from AGENTS.md bind
here and are checked when the values land:

* A state is never signalled by colour alone, so the dark palette keeps the
  fill-against-outline and icon cues intact; a token whose only job is to
  carry a state is not reused for a second state in dark.
* A disabled control must not look like a negative one: `primaryDisabled` and
  `settingsSwitchOff` stay distinguishable from each other in dark, not
  collapsed into the same grey because the background moved.
