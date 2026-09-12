
import 'package:flutter/material.dart';



class ColorThemeExtension extends ThemeExtension<ColorThemeExtension> {
  const ColorThemeExtension({
    required this.border,
    required this.border2,
    required this.border3,
    required this.highlight,
    required this.drag_indicator,
    required this.shadow,
    required this.errorBannerBg,
    required this.me,
    required this.toastBg,
    required this.toastText,
    required this.divider,
  });

  final Color? border;
  final Color? border2;
  final Color? border3;
  final Color? highlight;
  final Color? drag_indicator;
  final Color? shadow;
  final Color? errorBannerBg;
  final Color? me;
  final Color? toastBg;
  final Color? toastText;
  final Color? divider;

  static final light = ColorThemeExtension(
    border: Color(0xFFCCCCCC),
    border2: Color(0xFFBBBBBB),
    border3: Colors.black26,
    highlight: Color(0xFFE5E5E5),
    drag_indicator: Colors.grey[800],
    shadow: Colors.black,
    errorBannerBg: Color(0xFFFDEEEB),
    me: Colors.green,
    toastBg: Colors.black.withOpacity(0.6),
    toastText: Colors.white,
    divider: Colors.black38,
  );

  static final dark = ColorThemeExtension(
    border: Color(0xFF555555),
    border2: Color(0xFFE5E5E5),
    border3: Colors.white24,
    highlight: Color(0xFF3F3F3F),
    drag_indicator: Colors.grey,
    shadow: Colors.grey,
    errorBannerBg: Color(0xFF470F2D),
    me: Colors.greenAccent,
    toastBg: Colors.white.withOpacity(0.6),
    toastText: Colors.black,
    divider: Colors.white38,
  );

  @override
  ThemeExtension<ColorThemeExtension> copyWith({
    Color? border,
    Color? border2,
    Color? border3,
    Color? highlight,
    Color? drag_indicator,
    Color? shadow,
    Color? errorBannerBg,
    Color? me,
    Color? toastBg,
    Color? toastText,
    Color? divider,
  }) {
    return ColorThemeExtension(
      border: border ?? this.border,
      border2: border2 ?? this.border2,
      border3: border3 ?? this.border3,
      highlight: highlight ?? this.highlight,
      drag_indicator: drag_indicator ?? this.drag_indicator,
      shadow: shadow ?? this.shadow,
      errorBannerBg: errorBannerBg ?? this.errorBannerBg,
      me: me ?? this.me,
      toastBg: toastBg ?? this.toastBg,
      toastText: toastText ?? this.toastText,
      divider: divider ?? this.divider,
    );
  }

  @override
  ThemeExtension<ColorThemeExtension> lerp(
      ThemeExtension<ColorThemeExtension>? other, double t) {
    if (other is! ColorThemeExtension) {
      return this;
    }
    return ColorThemeExtension(
      border: Color.lerp(border, other.border, t),
      border2: Color.lerp(border2, other.border2, t),
      border3: Color.lerp(border3, other.border3, t),
      highlight: Color.lerp(highlight, other.highlight, t),
      drag_indicator: Color.lerp(drag_indicator, other.drag_indicator, t),
      shadow: Color.lerp(shadow, other.shadow, t),
      errorBannerBg: Color.lerp(shadow, other.errorBannerBg, t),
      me: Color.lerp(shadow, other.me, t),
      toastBg: Color.lerp(shadow, other.toastBg, t),
      toastText: Color.lerp(shadow, other.toastText, t),
      divider: Color.lerp(shadow, other.divider, t),
    );
  }
}
