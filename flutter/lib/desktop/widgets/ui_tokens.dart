import 'package:flutter/material.dart';

/// Spacing and typography tokens of the desktop home shell, in logical
/// pixels on an 8pt grid (docs: openuu-smoke/design-review.md). Pages use
/// these instead of bare numbers.
class UiSpace {
  static const double s1 = 4;
  static const double s2 = 8;
  static const double s3 = 12;
  static const double s4 = 16;
  static const double s5 = 20;
  static const double s6 = 24;
  static const double s8 = 32;
  static const double s10 = 40;
  static const double s12 = 48;

  // Layout.
  static const double sidebarWidth = 200;
  static const double sidebarPaddingX = s2;
  static const double sidebarPaddingTop = s3;
  static const double sidebarGroupLabelHeight = 32;
  static const double sidebarGroupGap = s4;
  static const double sidebarItemHeight = 36;
  static const double sidebarItemGap = 2;
  static const double sidebarIndentL1 = s3;
  static const double sidebarIndentL2 = s6;
  static const double sidebarIconSize = 16;
  static const double sidebarIconGap = s2;
  static const double sidebarRadius = 6;
  static const double sidebarFooterDividerGap = s2;
  static const double pagePaddingX = s8;
  static const double pagePaddingTop = s6;
  static const double pagePaddingBottom = s6;
  static const double contentMaxWidth = 1040;

  // Titles and groups.
  static const double pageTitleMarginBottom = s6;
  static const double groupHeaderHeight = 24;
  static const double groupHeaderMarginTop = s8;
  static const double groupHeaderMarginBottom = s4;
  static const double groupChevronSize = 14;
  static const double groupChevronGap = s2;
  static const double groupCountGap = s1;
  static const double sectionCardGap = s4;

  // Cards.
  static const double rowCardHeight = 56;
  static const double rowCardPaddingX = s3;
  static const double rowCardRadius = 8;
  static const double rowCardGap = s2;
  static const double rowIconSize = 32;
  static const double rowIconRadius = 8;
  static const double rowIconGap = s3;
  static const double rowBadgeGap = s2;
  static const double rowMetaGap = s3;
  static const double rowActionIconSize = 16;
  static const double rowActionHitSize = 32;
  static const double rowActionGap = s1;
  static const double sectionCardPadding = s5;
  static const double sectionCardRadius = 10;
  static const double sectionCardHeaderHeight = 48;
  static const double emptyStateHeight = 56;

  // Controls.
  static const double controlHeight = 32;
  static const double buttonPaddingX = s4;
  static const double buttonRadius = 6;
  static const double inputRadius = 6;
  static const double inputPaddingX = s3;
  static const double fieldGap = s2;
  static const double fieldLabelGap = 6;
  static const double menuItemHeight = 32;
  static const double menuPaddingY = s1;
  static const double menuItemPaddingX = s3;
  static const double menuOffset = s1;
  static const double menuRadius = 8;
  static const double statusDotSize = 6;
  static const double statusDotGap = 6;
  static const double tagHeight = 20;
  static const double tagPaddingX = 6;
  static const double tagRadius = 4;

  // Settings page (design-review-settings.md §2).
  static const double settingsContentMaxWidth = 760;
  static const double settingsTabBarHeight = 36;
  static const double settingsTabGap = s6;
  static const double settingsScrollPaddingY = s6;
  static const double settingsGroupTitleHeight = 20;
  static const double settingsGroupTitleGap = s2;
  static const double settingsGroupGap = s6;
  static const double settingsGroupRadius = 8;
  static const double settingsRowHeight = 48;
  static const double settingsRowHeightSub = 56;
  static const double settingsRowPaddingX = s4;
  static const double settingsControlGap = s3;
  static const double settingsChildIndent = 20;
  static const double settingsChildRail = 2;
  static const double settingsDisabledOpacity = .5;
  static const double settingsSwitchWidth = 36;
  static const double settingsSwitchHitHeight = 28;
  static const double settingsControlHeight = 28;
  static const double settingsDropdownPaddingLeft = 10;
  static const double settingsDropdownPaddingRight = s2;
  static const double settingsStatusBarHeight = 44;
}

class UiColor {
  static const Color text = Color(0xff1f2329);
  static const Color textSecondary = Color(0xff4e5969);
  static const Color muted = Color(0xff8a8f99);
  static const Color faint = Color(0xffb2b8c2);
  static const Color primary = Color(0xff2b6cf6);
  static const Color primaryTint = Color(0xffeaf1ff);
  static const Color border = Color(0xffe5e6eb);
  static const Color borderHover = Color(0xffc9d4f0);
  static const Color surfaceHover = Color(0xfff5f8ff);
  static const Color ready = Color(0xff22c55e);
  static const Color favorite = Color(0xffffb020);
  static const Color settingsDivider = Color(0xfff0f1f3);
  static const Color settingsRowHover = Color(0xfff7f8fa);
  static const Color settingsSwitchOff = Color(0xffc9cdd4);
  static const Color inputBorder = Color(0xffdcdfe6);
  static const Color statusRunningBg = Color(0xfff2f8f2);
  static const Color statusStoppedBg = Color(0xfffff7e8);
}

class UiType {
  static const List<String> fallback = [
    'PingFang SC',
    'Microsoft YaHei UI',
    'Noto Sans SC',
    'Segoe UI',
  ];
  static const TextStyle _base =
      TextStyle(fontFamily: 'Microsoft YaHei', fontFamilyFallback: fallback);

  static final TextStyle pageTitle = _base.copyWith(
      fontSize: 20, fontWeight: FontWeight.w600, height: 28 / 20, color: UiColor.text);
  static final TextStyle sectionTitle = _base.copyWith(
      fontSize: 15, fontWeight: FontWeight.w600, height: 22 / 15, color: UiColor.text);
  static final TextStyle groupTitle = _base.copyWith(
      fontSize: 13, fontWeight: FontWeight.w500, height: 20 / 13, color: UiColor.muted);
  static final TextStyle groupCount = _base.copyWith(
      fontSize: 12, fontWeight: FontWeight.w400, height: 20 / 12, color: UiColor.faint);
  static final TextStyle rowTitle = _base.copyWith(
      fontSize: 14, fontWeight: FontWeight.w500, height: 22 / 14, color: UiColor.text);
  static final TextStyle sidebarItem = _base.copyWith(
      fontSize: 13, fontWeight: FontWeight.w400, height: 20 / 13, color: UiColor.textSecondary);
  static final TextStyle sidebarItemSelected =
      sidebarItem.copyWith(fontWeight: FontWeight.w500, color: UiColor.primary);
  static final TextStyle sidebarGroup = _base.copyWith(
      fontSize: 12, fontWeight: FontWeight.w500, height: 20 / 12, color: UiColor.muted);
  static final TextStyle caption = _base.copyWith(
      fontSize: 12, fontWeight: FontWeight.w400, height: 18 / 12, color: UiColor.muted);
  static final TextStyle tag = _base.copyWith(
      fontSize: 11, fontWeight: FontWeight.w500, height: 16 / 11, color: UiColor.primary);
  static final TextStyle deviceId = _base.copyWith(
      fontSize: 28, fontWeight: FontWeight.w600, height: 36 / 28, letterSpacing: 2, color: UiColor.text);
  static final TextStyle button =
      _base.copyWith(fontSize: 13, fontWeight: FontWeight.w500);
}
