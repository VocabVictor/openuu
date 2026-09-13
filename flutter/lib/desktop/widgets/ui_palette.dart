import 'package:flutter/material.dart';

import 'ui_tokens.dart';

/// The design tokens' colours, in the appearance in force.
///
/// [UiColor]'s own members are compile-time constants of the light values and
/// stay that way: they are what an unmigrated file still reads and what a
/// widget with no context falls back to. A file joins the dark appearance by
/// taking `UiColor.of(context)` and reading the same names from it
/// (docs/dark-token-decision.md).
///
/// The dark values are not the light ones inverted. Two rules from AGENTS.md
/// bind here: a state is never told apart by colour alone, and a disabled
/// control must not look like a negative one, which is why
/// [primaryDisabled] stays a dimmed blue while [settingsSwitchOff] stays a
/// grey rather than both collapsing into the background.
class UiPalette extends ThemeExtension<UiPalette> {
  const UiPalette({
    required this.text,
    required this.textSecondary,
    required this.muted,
    required this.faint,
    required this.primary,
    required this.primaryFill,
    required this.primaryTint,
    required this.primaryTintHover,
    required this.dangerTint,
    required this.dangerTintHover,
    required this.border,
    required this.borderHover,
    required this.surface,
    required this.surfaceHover,
    required this.panelBg,
    required this.inverseSurface,
    required this.onInverseSurface,
    required this.onPrimary,
    required this.ready,
    required this.favorite,
    required this.settingsDivider,
    required this.settingsRowHover,
    required this.settingsSwitchOff,
    required this.inputBorder,
    required this.statusRunningBg,
    required this.statusStoppedBg,
    required this.danger,
    required this.dangerFill,
    required this.onWarning,
    required this.dangerBorder,
    required this.primaryDisabled,
    required this.warning,
    required this.success,
  });

  final Color text;
  final Color textSecondary;
  final Color muted;
  final Color faint;
  final Color primary;

  /// The fill under [onPrimary] content. Darker than [primary] in the dark
  /// appearance, where a colour light enough to read as text on the dark
  /// surface is too light to carry white text (docs/dark-acceptance.md).
  final Color primaryFill;
  final Color primaryTint;
  final Color primaryTintHover;
  final Color dangerTint;
  final Color dangerTintHover;
  final Color border;
  final Color borderHover;
  final Color surface;
  final Color surfaceHover;
  final Color panelBg;

  /// A surface deliberately opposite to the page: dark in the light
  /// appearance, light in the dark one. For the badge that has to stand
  /// out of the page rather than sit on it.
  final Color inverseSurface;
  final Color onInverseSurface;
  final Color onPrimary;
  final Color ready;
  final Color favorite;
  final Color settingsDivider;
  final Color settingsRowHover;
  final Color settingsSwitchOff;
  final Color inputBorder;
  final Color statusRunningBg;
  final Color statusStoppedBg;
  final Color danger;

  /// The fill under [onPrimary] content for a destructive action. Darker than
  /// [danger], which is a text colour: white on the bright red measures
  /// 2.93:1 in dark and 3.71:1 in light, under what a label needs.
  final Color dangerFill;

  /// Content on the [warning] fill. Warning stays a bright amber in both
  /// appearances, so what sits on it is dark in both: white on it is 2.12:1.
  final Color onWarning;
  final Color dangerBorder;
  final Color primaryDisabled;
  final Color warning;
  final Color success;

  static const UiPalette light = UiPalette(
    text: Color(0xff1f2329),
    textSecondary: Color(0xff4e5969),
    muted: Color(0xff8a8f99),
    faint: Color(0xffb2b8c2),
    primary: Color(0xff2b6cf6),
    primaryFill: Color(0xff2b6cf6),
    primaryTint: Color(0xffeaf1ff),
    primaryTintHover: Color(0xffdce8ff),
    dangerTint: Color(0xfffff0ef),
    dangerTintHover: Color(0xffffe1df),
    border: Color(0xffe5e6eb),
    borderHover: Color(0xffc9d4f0),
    surface: Color(0xffffffff),
    surfaceHover: Color(0xfff5f8ff),
    panelBg: Color(0xfffafbfc),
    inverseSurface: Color(0xff20262d),
    onInverseSurface: Color(0xffffffff),
    onPrimary: Color(0xffffffff),
    ready: Color(0xff22c55e),
    favorite: Color(0xffffb020),
    settingsDivider: Color(0xfff0f1f3),
    settingsRowHover: Color(0xfff7f8fa),
    settingsSwitchOff: Color(0xffc9cdd4),
    inputBorder: Color(0xffdcdfe6),
    statusRunningBg: Color(0xfff2f8f2),
    statusStoppedBg: Color(0xfffff7e8),
    danger: Color(0xfff53f3f),
    dangerFill: Color(0xffd32f2f),
    onWarning: Color(0xff1f2329),
    dangerBorder: Color(0xfffbaca3),
    primaryDisabled: Color(0xffb8d0ff),
    warning: Color(0xffff7d00),
    success: Color(0xff00b42a),
  );

  static const UiPalette dark = UiPalette(
    text: Color(0xffe5e6eb),
    textSecondary: Color(0xffc9cdd4),
    muted: Color(0xff9296a0),
    faint: Color(0xff6b7079),
    primary: Color(0xff5d90ff),
    primaryFill: Color(0xff2f6ae0),
    primaryTint: Color(0xff1b2a4a),
    primaryTintHover: Color(0xff24375e),
    dangerTint: Color(0xff3a201f),
    dangerTintHover: Color(0xff4d2724),
    border: Color(0xff33363d),
    borderHover: Color(0xff3d4a66),
    surface: Color(0xff23262e),
    surfaceHover: Color(0xff2a2e38),
    panelBg: Color(0xff1c1f24),
    inverseSurface: Color(0xffe5e6eb),
    onInverseSurface: Color(0xff1c1f24),
    onPrimary: Color(0xffffffff),
    ready: Color(0xff3ddc84),
    favorite: Color(0xffffc043),
    settingsDivider: Color(0xff2a2d33),
    settingsRowHover: Color(0xff2a2e38),
    settingsSwitchOff: Color(0xff5f636b),
    inputBorder: Color(0xff3a3e46),
    statusRunningBg: Color(0xff14301f),
    statusStoppedBg: Color(0xff33280f),
    danger: Color(0xfff76965),
    dangerFill: Color(0xffd32f2f),
    onWarning: Color(0xff1f2329),
    dangerBorder: Color(0xff6b3a36),
    primaryDisabled: Color(0xff2d3d5c),
    warning: Color(0xffff9a2e),
    success: Color(0xff23c343),
  );

  @override
  UiPalette copyWith({
    Color? text,
    Color? textSecondary,
    Color? muted,
    Color? faint,
    Color? primary,
    Color? primaryFill,
    Color? primaryTint,
    Color? primaryTintHover,
    Color? dangerTint,
    Color? dangerTintHover,
    Color? border,
    Color? borderHover,
    Color? surface,
    Color? surfaceHover,
    Color? panelBg,
    Color? inverseSurface,
    Color? onInverseSurface,
    Color? onPrimary,
    Color? ready,
    Color? favorite,
    Color? settingsDivider,
    Color? settingsRowHover,
    Color? settingsSwitchOff,
    Color? inputBorder,
    Color? statusRunningBg,
    Color? statusStoppedBg,
    Color? danger,
    Color? dangerFill,
    Color? onWarning,
    Color? dangerBorder,
    Color? primaryDisabled,
    Color? warning,
    Color? success,
  }) =>
      UiPalette(
        text: text ?? this.text,
        textSecondary: textSecondary ?? this.textSecondary,
        muted: muted ?? this.muted,
        faint: faint ?? this.faint,
        primary: primary ?? this.primary,
        primaryFill: primaryFill ?? this.primaryFill,
        primaryTint: primaryTint ?? this.primaryTint,
        primaryTintHover: primaryTintHover ?? this.primaryTintHover,
        dangerTint: dangerTint ?? this.dangerTint,
        dangerTintHover: dangerTintHover ?? this.dangerTintHover,
        border: border ?? this.border,
        borderHover: borderHover ?? this.borderHover,
        surface: surface ?? this.surface,
        surfaceHover: surfaceHover ?? this.surfaceHover,
        panelBg: panelBg ?? this.panelBg,
        inverseSurface: inverseSurface ?? this.inverseSurface,
        onInverseSurface: onInverseSurface ?? this.onInverseSurface,
        onPrimary: onPrimary ?? this.onPrimary,
        ready: ready ?? this.ready,
        favorite: favorite ?? this.favorite,
        settingsDivider: settingsDivider ?? this.settingsDivider,
        settingsRowHover: settingsRowHover ?? this.settingsRowHover,
        settingsSwitchOff: settingsSwitchOff ?? this.settingsSwitchOff,
        inputBorder: inputBorder ?? this.inputBorder,
        statusRunningBg: statusRunningBg ?? this.statusRunningBg,
        statusStoppedBg: statusStoppedBg ?? this.statusStoppedBg,
        danger: danger ?? this.danger,
        dangerFill: dangerFill ?? this.dangerFill,
        onWarning: onWarning ?? this.onWarning,
        dangerBorder: dangerBorder ?? this.dangerBorder,
        primaryDisabled: primaryDisabled ?? this.primaryDisabled,
        warning: warning ?? this.warning,
        success: success ?? this.success,
      );

  @override
  UiPalette lerp(ThemeExtension<UiPalette>? other, double t) {
    if (other is! UiPalette) return this;
    return UiPalette(
      text: Color.lerp(text, other.text, t) ?? text,
      textSecondary: Color.lerp(textSecondary, other.textSecondary, t) ?? textSecondary,
      muted: Color.lerp(muted, other.muted, t) ?? muted,
      faint: Color.lerp(faint, other.faint, t) ?? faint,
      primary: Color.lerp(primary, other.primary, t) ?? primary,
      primaryFill: Color.lerp(primaryFill, other.primaryFill, t) ?? primaryFill,
      primaryTint: Color.lerp(primaryTint, other.primaryTint, t) ?? primaryTint,
      primaryTintHover: Color.lerp(primaryTintHover, other.primaryTintHover, t) ?? primaryTintHover,
      dangerTint: Color.lerp(dangerTint, other.dangerTint, t) ?? dangerTint,
      dangerTintHover: Color.lerp(dangerTintHover, other.dangerTintHover, t) ?? dangerTintHover,
      border: Color.lerp(border, other.border, t) ?? border,
      borderHover: Color.lerp(borderHover, other.borderHover, t) ?? borderHover,
      surface: Color.lerp(surface, other.surface, t) ?? surface,
      surfaceHover: Color.lerp(surfaceHover, other.surfaceHover, t) ?? surfaceHover,
      panelBg: Color.lerp(panelBg, other.panelBg, t) ?? panelBg,
      inverseSurface: Color.lerp(inverseSurface, other.inverseSurface, t) ?? inverseSurface,
      onInverseSurface: Color.lerp(onInverseSurface, other.onInverseSurface, t) ?? onInverseSurface,
      onPrimary: Color.lerp(onPrimary, other.onPrimary, t) ?? onPrimary,
      ready: Color.lerp(ready, other.ready, t) ?? ready,
      favorite: Color.lerp(favorite, other.favorite, t) ?? favorite,
      settingsDivider: Color.lerp(settingsDivider, other.settingsDivider, t) ?? settingsDivider,
      settingsRowHover: Color.lerp(settingsRowHover, other.settingsRowHover, t) ?? settingsRowHover,
      settingsSwitchOff: Color.lerp(settingsSwitchOff, other.settingsSwitchOff, t) ?? settingsSwitchOff,
      inputBorder: Color.lerp(inputBorder, other.inputBorder, t) ?? inputBorder,
      statusRunningBg: Color.lerp(statusRunningBg, other.statusRunningBg, t) ?? statusRunningBg,
      statusStoppedBg: Color.lerp(statusStoppedBg, other.statusStoppedBg, t) ?? statusStoppedBg,
      danger: Color.lerp(danger, other.danger, t) ?? danger,
      dangerFill: Color.lerp(dangerFill, other.dangerFill, t) ?? dangerFill,
      onWarning: Color.lerp(onWarning, other.onWarning, t) ?? onWarning,
      dangerBorder: Color.lerp(dangerBorder, other.dangerBorder, t) ?? dangerBorder,
      primaryDisabled: Color.lerp(primaryDisabled, other.primaryDisabled, t) ?? primaryDisabled,
      warning: Color.lerp(warning, other.warning, t) ?? warning,
      success: Color.lerp(success, other.success, t) ?? success,
    );
  }
}

/// The type tokens with their colours taken from the palette in force, so a
/// migrated file never pairs themed colours with a fixed text colour.
class UiTypeset {
  const UiTypeset(this.palette);

  final UiPalette palette;

  TextStyle get pageTitle => UiType.pageTitle.copyWith(color: palette.text);
  TextStyle get sectionTitle =>
      UiType.sectionTitle.copyWith(color: palette.text);
  TextStyle get groupTitle => UiType.groupTitle.copyWith(color: palette.muted);
  TextStyle get groupCount => UiType.groupCount.copyWith(color: palette.faint);
  TextStyle get rowTitle => UiType.rowTitle.copyWith(color: palette.text);
  TextStyle get sidebarItem =>
      UiType.sidebarItem.copyWith(color: palette.textSecondary);
  TextStyle get sidebarItemSelected =>
      UiType.sidebarItemSelected.copyWith(color: palette.primary);
  TextStyle get sidebarGroup =>
      UiType.sidebarGroup.copyWith(color: palette.muted);
  TextStyle get caption => UiType.caption.copyWith(color: palette.muted);
  TextStyle get tag => UiType.tag.copyWith(color: palette.primary);
  TextStyle get deviceId => UiType.deviceId.copyWith(color: palette.text);
  TextStyle get button => UiType.button;
}
