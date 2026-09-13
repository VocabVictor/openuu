import 'dart:math' as math;
import 'package:flutter/material.dart';
import '../../widgets/ui_tokens.dart';

part 'devices_painter.dart';

/// OpenUU's signed-out landing page. Authentication and navigation stay in the
/// existing desktop shell; this widget only owns the presentation.
class DesktopWelcomePage extends StatelessWidget {
  final VoidCallback onLogin;
  final VoidCallback onAssistance;
  final VoidCallback onFavorites;
  final VoidCallback? onSettings;
  final VoidCallback? onDevices;
  final Widget? content;
  final bool settingsSelected;
  final bool assistanceSelected;
  final bool favoritesSelected;
  final Widget? header;
  final Widget? deviceItem;
  const DesktopWelcomePage(
      {super.key,
      required this.onLogin,
      required this.onAssistance,
      required this.onFavorites,
      this.onSettings,
      this.onDevices,
      this.content,
      this.header,
      this.deviceItem,
      this.assistanceSelected = false,
      this.favoritesSelected = false,
      this.settingsSelected = false});

  static const blue = UiColor.of(context).primary;

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    return Material(
      color: const Color(0xffeff4f7),
      child: Column(children: [
        Expanded(child: LayoutBuilder(builder: (context, constraints) {
          return Row(children: [
            SizedBox(
                width: UiSpace.sidebarWidth,
                child: Column(children: [
                  const SizedBox(height: UiSpace.sidebarPaddingTop),
                  _group(t('我的设备', 'My devices'), Icons.devices_outlined),
                  if (deviceItem != null) deviceItem!,
                  _item(
                      t('全部设备', 'All devices'),
                      Icons.grid_view_rounded,
                      !settingsSelected &&
                          !assistanceSelected &&
                          !favoritesSelected &&
                          deviceItem == null,
                      onDevices ?? () {}),
                  _item(t('收藏设备', 'Favorites'), Icons.bookmark,
                      favoritesSelected, onFavorites),
                  const SizedBox(height: UiSpace.sidebarGroupGap),
                  _group(t('远程协助', 'Remote assistance'), Icons.crop_free),
                  _item(t('开始协助', 'Start assistance'), Icons.screen_share,
                      assistanceSelected, onAssistance),
                  const Spacer(),
                  Divider(height: 1, color: UiColor.of(context).border),
                  const SizedBox(height: UiSpace.sidebarFooterDividerGap),
                  if (onSettings != null)
                    _item(t('设置', 'Settings'), Icons.settings_outlined,
                        settingsSelected, onSettings!),
                  const SizedBox(height: UiSpace.s3),
                ])),
            Expanded(
                child: Container(
                    decoration: BoxDecoration(
                        color: const Color(0xfff8fbfd),
                        border: Border.all(color: const Color(0xffdfe5e9)),
                        borderRadius: const BorderRadius.only(
                            topLeft: Radius.circular(14))),
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Padding(
                              padding: const EdgeInsets.fromLTRB(
                                  UiSpace.pagePaddingX,
                                  UiSpace.pagePaddingTop,
                                  UiSpace.pagePaddingX,
                                  UiSpace.pageTitleMarginBottom),
                              child: header ??
                                  Text(
                                      settingsSelected
                                          ? t('设置', 'Settings')
                                          : favoritesSelected
                                              ? t('收藏设备', 'Favorites')
                                              : t('全部设备', 'All devices'),
                                      style: UiType.of(context).pageTitle)),
                          Expanded(child: content ?? _welcomeContent(zh)),
                        ]))),
          ]);
        })),
      ]),
    );
  }

  Widget _welcomeContent(bool zh) => LayoutBuilder(builder: (context, bounds) {
        final inset = UiSpace.pagePaddingX;
        final gap = (bounds.maxHeight * .035).clamp(10.0, 28.0);
        final contentWidth = math.max(0.0, bounds.maxWidth - inset * 2);
        // Reserve room for a two-line caption, the button and all vertical gaps.
        // Both axes constrain the artwork, including short ultrawide windows.
        final imageHeight = math.min(
          math.min(contentWidth * .78, 640.0) / 1.6,
          math.max(64.0, bounds.maxHeight - 180.0 - gap * 2),
        );
        final textStyle = UiType.of(context).rowTitle.copyWith(
            fontWeight: FontWeight.w400, height: 1.6, fontSize: 15);
        return Center(
            child: SingleChildScrollView(
          padding: EdgeInsets.symmetric(horizontal: inset, vertical: 16),
          child: SizedBox(
              width: contentWidth,
              child: Column(mainAxisSize: MainAxisSize.min, children: [
                SizedBox(
                    key: const ValueKey('welcome-artwork'),
                    width: imageHeight * 1.6,
                    height: imageHeight,
                    child: const CustomPaint(painter: _DevicesPainter())),
                SizedBox(height: gap),
                Text(
                    zh
                        ? '随时访问个人设备，可用于办公、游戏、文件传输。'
                        : 'Access your devices for work, games and file transfers.',
                    textAlign: TextAlign.center,
                    style: textStyle),
                SizedBox(height: gap),
                FilledButton(
                    key: const ValueKey('welcome-login'),
                    onPressed: onLogin,
                    style: FilledButton.styleFrom(
                        backgroundColor: blue,
                        foregroundColor: Colors.white,
                        padding: const EdgeInsets.symmetric(
                            horizontal: 26, vertical: 11),
                        shape: RoundedRectangleBorder(
                            borderRadius:
                                BorderRadius.circular(UiSpace.buttonRadius))),
                    child: Text(zh ? '立即登录' : 'Sign in',
                        style: textStyle.copyWith(color: Colors.white))),
                SizedBox(height: gap),
              ])),
        ));
      });

  Widget _group(String title, IconData icon) => SizedBox(
      height: UiSpace.sidebarGroupLabelHeight,
      child: Padding(
          padding: const EdgeInsets.symmetric(
              horizontal: UiSpace.sidebarPaddingX + UiSpace.sidebarIndentL1),
          child: Row(children: [
            Icon(icon, size: UiSpace.sidebarIconSize, color: UiColor.of(context).muted),
            const SizedBox(width: UiSpace.sidebarIconGap),
            Expanded(child: Text(title, style: UiType.of(context).sidebarGroup)),
            Icon(Icons.keyboard_arrow_up,
                size: 16, color: UiColor.of(context).faint)
          ])));

  Widget _item(
          String title, IconData icon, bool selected, VoidCallback onTap) =>
      Padding(
          padding: const EdgeInsets.symmetric(
              horizontal: UiSpace.sidebarPaddingX,
              vertical: UiSpace.sidebarItemGap / 2),
          child: Material(
              color: selected ? const Color(0xffe1e8ec) : Colors.transparent,
              borderRadius: BorderRadius.circular(UiSpace.sidebarRadius),
              child: InkWell(
                  onTap: onTap,
                  borderRadius: BorderRadius.circular(UiSpace.sidebarRadius),
                  child: SizedBox(
                      height: UiSpace.sidebarItemHeight,
                      child: Row(children: [
                        Container(
                            width: 3,
                            height: 16,
                            decoration: BoxDecoration(
                                color:
                                    selected ? blue : Colors.transparent,
                                borderRadius: BorderRadius.circular(3))),
                        const SizedBox(width: UiSpace.sidebarIndentL2 - 3),
                        Icon(icon, color: blue, size: UiSpace.sidebarIconSize),
                        const SizedBox(width: UiSpace.sidebarIconGap),
                        Expanded(
                            child: Text(title,
                                style: selected
                                    ? UiType.of(context).sidebarItemSelected
                                    : UiType.of(context).sidebarItem)),
                      ])))));
}
