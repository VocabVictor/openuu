import 'package:flutter/material.dart';

import 'ui_palette.dart';

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
  static const double settingsBrandHeadHeight = 72;
  static const double settingsBrandIconSize = 48;
  static const double settingsNumberFieldWidth = 56;
  static const double settingsSliderWidth = 160;

  // Expand panels (design-review-settings.md §2.6).
  static const double panelPaddingLeft = s8;
  static const double panelPaddingRight = s4;
  static const double panelPaddingY = s4;
  static const double panelFieldGap = s4;
  static const double panelFieldWidth = 320;
  static const double panelFooterGap = s4;
  static const double panelErrorHeight = 16;
  static const double panelTextAreaHeight = 64;
  static const Duration panelDuration = Duration(milliseconds: 180);
  static const Duration panelFadeDuration = Duration(milliseconds: 120);

  // Dialogs (design-review-settings.md §2.6): 400 wide with 24 padding.
  static const double dialogContentWidth = 352;
  static const double dialogTitleHeight = 48;
}

class UiColor {
  /// The palette of the appearance in force. A file migrates by taking this
  /// once at the top of `build` and reading the same names from it; the
  /// constants below stay as the light values for anything not migrated and
  /// for a caller with no context (docs/dark-token-decision.md).
  static UiPalette of(BuildContext context) =>
      Theme.of(context).extension<UiPalette>() ?? UiPalette.light;

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
  static const Color panelBg = Color(0xfffafbfc);
  static const Color danger = Color(0xfff53f3f);
  static const Color primaryDisabled = Color(0xffb8d0ff);
  static const Color dangerBorder = Color(0xfffbaca3);
  static const Color warning = Color(0xffff7d00);
  static const Color success = Color(0xff00b42a);
  static const Color surface = Color(0xffffffff);
  static const Color onPrimary = Color(0xffffffff);
}

class UiType {
  /// The type tokens with the palette's colours; the pair of [UiColor.of].
  static UiTypeset of(BuildContext context) => UiTypeset(UiColor.of(context));

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

/// Session windows (remote control, file transfer, terminal, port forward,
/// camera): docs/session-window-restyle.md. Sizes only; colours and type come
/// from [UiColor] / [UiType].
class UiSession {
  // tab bar of a session window (the main window keeps the DesktopTab defaults)
  static const double tabBarHeight = 36;
  static const double tabPaddingX = UiSpace.s3;
  static const double tabIconSize = 16;
  static const double tabIconGap = UiSpace.s2;
  static const double tabIndicator = 2;
  static const double tabCloseSize = 16;
  static const double tabCloseHitSize = 24;
  static const double tabActionHitSize = 36;
  static const double tabActionIconSize = 14;
  static const double tabMaxLabelWidth = 200;

  // floating toolbar
  static const double toolbarRadius = 8;
  static const double toolbarPaddingX = UiSpace.s2;
  static const double toolbarButtonSize = 28;
  static const double toolbarIconSize = 16;
  static const double toolbarButtonGap = UiSpace.s1;
  static const double toolbarButtonRadius = 6;
  static const double toolbarHandleThickness = 20;
  static const double toolbarHandleIconSize = 14;
  static const double toolbarMenuMinWidth = 200;
  static const Color toolbarShadow = Color(0x1a000000);
  static const Offset toolbarShadowOffset = Offset(0, 4);
  static const double toolbarShadowBlur = 16;

  // inline status bar at the top of the canvas
  // 32, not 28: the inline reconnect / disconnect buttons need a 24 hit box
  // to stay comfortably clickable, and 28 cannot hold one with padding.
  static const double statusBarHeight = 32;
  static const double statusBarButtonHeight = 24;
  static const double statusBarPaddingX = UiSpace.s3;
  static const double statusBarHotZone = 8;
  static const Duration statusBarAutoHide = Duration(seconds: 3);
  static const Duration statusBarFade = Duration(milliseconds: 150);

  // session dialogs (password, 2FA, waiting, relay hint, restart)
  static const double dialogWidth = UiSpace.dialogContentWidth;
  static const double dialogPadding = UiSpace.s6;
  static const double dialogIconSize = 16;
  static const double dialogFieldGap = UiSpace.s4;
  static const double dialogButtonGap = UiSpace.s2;
  static const double dialogControlHeight = UiSpace.controlHeight;
}

/// Connection manager, the window the controlled side sees when someone asks
/// to control this device (docs/cm-restyle-plan.md). Sizes only; colours and
/// type come from [UiColor] / [UiType]. This is a consent surface: see the
/// rule in AGENTS.md before changing how the accept and reject controls look.
class UiCm {
  // request banner
  static const double bannerPaddingX = UiSpace.s4;
  static const double bannerPaddingY = UiSpace.s4;
  static const double bannerGap = UiSpace.s3;
  static const double avatarSize = UiSpace.s12;
  static const double avatarRadius = UiSpace.s2;
  static const double avatarInitialSize = 22;
  static const double appIconSize = 30;
  static const double titleBarIconSize = 18;
  static const double titleBarPaddingX = UiSpace.s1;

  // accept / reject controls
  static const double controlBarPaddingX = UiSpace.s4;
  static const double controlBarPaddingY = UiSpace.s3;
  static const double controlGap = UiSpace.s3;
  static const double controlHeight = UiSpace.controlHeight;
  static const double controlRadius = 6;
  static const double controlMinWidth = 96;
  static const double controlIconSize = 16;

  // permission board and transfer log
  static const double boardPadding = UiSpace.s4;
  static const double boardRowHeight = UiSpace.s8;
  static const double boardRowGap = UiSpace.s2;
  static const double boardIconSize = 18;
  static const double logRowHeight = UiSpace.s10;
  static const double tabStripHeight = UiSpace.s10;
}

