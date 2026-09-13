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
   references sit in a `const` expression, and the compiler names every one.

A file with no `BuildContext` where the colour is needed (a top-level helper,
a `static` builder) takes the context as a parameter rather than reaching for
a global; if that is not practical, it keeps the `UiColor.x` constant and is
listed as unmigrated rather than being half-converted.

## What the palette must satisfy

The dark values are not the light ones inverted. Two rules from AGENTS.md bind
here and are checked when the values land:

* A state is never signalled by colour alone, so the dark palette keeps the
  fill-against-outline and icon cues intact; a token whose only job is to
  carry a state is not reused for a second state in dark.
* A disabled control must not look like a negative one: `primaryDisabled` and
  `settingsSwitchOff` stay distinguishable from each other in dark, not
  collapsed into the same grey because the background moved.
