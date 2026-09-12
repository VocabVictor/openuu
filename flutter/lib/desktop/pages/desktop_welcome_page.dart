import 'package:flutter_hbb/common/widgets/brand_icon.dart';
import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

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
  final bool showTitleBar;
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
      this.showTitleBar = true,
      this.settingsSelected = false});

  static const blue = Color(0xff3979ff);

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    return Material(
      color: const Color(0xffeff4f7),
      child: Column(children: [
        if (showTitleBar) SizedBox(
            height: 46,
            child: Row(children: [
              Expanded(
                  child: GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onPanStart: (_) => windowManager.startDragging(),
                      onDoubleTap: () async {
                        if (await windowManager.isMaximized()) {
                          await windowManager.unmaximize();
                        } else {
                          await windowManager.maximize();
                        }
                      },
                      child: const Padding(
                          padding: EdgeInsets.symmetric(horizontal: 24),
                          child: Row(children: [
                            BrandIcon(size: 20),
                            SizedBox(width: 12),
                            Text('OpenUU',
                                style: TextStyle(
                                    fontFamily: 'Microsoft YaHei',
                                    fontFamilyFallback: [
                                      'Segoe UI',
                                      'Noto Sans CJK SC'
                                    ],
                                    fontSize: 13,
                                    fontWeight: FontWeight.w500,
                                    color: Color(0xff172333)))
                          ])))),
              _titleBarButton(t('登录', 'Sign in'), Icons.person_outline, onLogin),
              if (onSettings != null)
                _titleBarButton(t('设置', 'Settings'), Icons.menu, onSettings!),
              _titleBarButton(t('最小化', 'Minimize'), Icons.remove,
                  () => windowManager.minimize()),
              _titleBarButton(t('关闭', 'Close'), Icons.close,
                  () => windowManager.close()),
              const SizedBox(width: 10),
            ])),
        Expanded(child: LayoutBuilder(builder: (context, constraints) {
          // Constraints are logical pixels; Flutter applies the monitor DPI once.
          final sidebarWidth = (constraints.maxWidth * .24).clamp(200.0, 300.0);
          final inset = (constraints.maxWidth * .035).clamp(24.0, 48.0);
          return Row(children: [
            SizedBox(
                width: sidebarWidth,
                child: Column(children: [
                  _group(t('我的设备', 'My devices'), Icons.devices_outlined),
                  if (deviceItem != null) deviceItem!,
                  _item(t('全部设备', 'All devices'), Icons.grid_view_rounded,
                      !settingsSelected && !assistanceSelected && deviceItem == null, onDevices ?? () {}),
                  _group(t('远程协助', 'Remote assistance'), Icons.crop_free),
                  _item(t('开始协助', 'Start assistance'), Icons.screen_share,
                      assistanceSelected, onAssistance),
                  _item(t('收藏设备', 'Favorites'), Icons.bookmark, false,
                      onFavorites),
                  const Spacer(),
                  const Divider(height: 1),
                  if (onSettings != null)
                    _item(t('设置', 'Settings'), Icons.settings_outlined,
                        settingsSelected, onSettings!),
                  const SizedBox(height: 12),
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
                              padding: EdgeInsets.fromLTRB(
                                  inset,
                                  constraints.maxHeight < 600 ? 20 : 30,
                                  inset,
                                  8),
                              child: header ?? Text(
                                  settingsSelected
                                      ? t('设置', 'Settings')
                                      : t('全部设备', 'All devices'),
                                  style: const TextStyle(
                                      fontFamily: 'Microsoft YaHei',
                                      fontFamilyFallback: [
                                        'Segoe UI',
                                        'Noto Sans CJK SC'
                                      ],
                                      fontSize: 24,
                                      fontWeight: FontWeight.w700,
                                      color: Color(0xff101820)))),
                          Expanded(child: content ?? _welcomeContent(zh)),
                        ]))),
          ]);
        })),
      ]),
    );
  }

  Widget _titleBarButton(String tooltip, IconData icon, VoidCallback onTap) {
    const shape = RoundedRectangleBorder(
        borderRadius: BorderRadius.all(Radius.circular(4)));
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 2),
      child: Tooltip(
        message: tooltip,
        child: SizedBox.square(
          dimension: 36,
          child: Material(
            color: Colors.transparent,
            shape: shape,
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              customBorder: shape,
              hoverColor: const Color(0x14000000),
              focusColor: const Color(0x14000000),
              highlightColor: const Color(0x1f000000),
              onTap: onTap,
              child: Center(child: Icon(icon, size: 16)),
            ),
          ),
        ),
      ),
    );
  }

  Widget _welcomeContent(bool zh) => LayoutBuilder(builder: (context, bounds) {
        final inset = (bounds.maxWidth * .04).clamp(16.0, 40.0);
        final gap = (bounds.maxHeight * .035).clamp(10.0, 28.0);
        final contentWidth = math.max(0.0, bounds.maxWidth - inset * 2);
        // Reserve room for a two-line caption, the button and all vertical gaps.
        // Both axes constrain the artwork, including short ultrawide windows.
        final imageHeight = math.min(
          math.min(contentWidth * .78, 640.0) / 1.6,
          math.max(64.0, bounds.maxHeight - 180.0 - gap * 2),
        );
        const textStyle = TextStyle(
            fontFamily: 'Microsoft YaHei',
            fontFamilyFallback: ['Segoe UI', 'Noto Sans CJK SC'],
            fontSize: 16,
            height: 1.6,
            color: Color(0xff172333));
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
                            borderRadius: BorderRadius.circular(5))),
                    child: Text(zh ? '立即登录' : 'Sign in',
                        style: textStyle.copyWith(color: Colors.white))),
                SizedBox(height: gap),
              ])),
        ));
      });

  Widget _group(String title, IconData icon) => Padding(
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 8),
      child: Row(children: [
        Icon(icon, size: 16, color: const Color(0xff263238)),
        const SizedBox(width: 12),
        Expanded(
            child: Text(title,
                style: const TextStyle(
                    fontFamily: 'Microsoft YaHei',
                    fontFamilyFallback: ['Segoe UI', 'Noto Sans CJK SC'],
                    fontSize: 14,
                    color: Color(0xff172333)))),
        const Icon(Icons.keyboard_arrow_up, size: 18, color: Colors.grey)
      ]));

  Widget _item(
          String title, IconData icon, bool selected, VoidCallback onTap) =>
      Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
          child: Material(
              color: selected ? const Color(0xffe1e8ec) : Colors.transparent,
              borderRadius: BorderRadius.circular(6),
              child: InkWell(
                  onTap: onTap,
                  borderRadius: BorderRadius.circular(6),
                  child: SizedBox(
                      height: 36,
                      child: Row(children: [
                        Container(
                            width: 3,
                            height: 20,
                            decoration: BoxDecoration(
                                color: selected ? blue : Colors.transparent,
                                borderRadius: BorderRadius.circular(3))),
                        const SizedBox(width: 30),
                        Icon(icon, color: blue, size: 18),
                        const SizedBox(width: 15),
                        Expanded(
                            child: Text(title,
                                style: const TextStyle(
                                    fontFamily: 'Microsoft YaHei',
                                    fontFamilyFallback: [
                                      'Segoe UI',
                                      'Noto Sans CJK SC'
                                    ],
                                    fontSize: 14,
                                    color: Color(0xff172333)))),
                      ])))));
}

/// Scalable product illustration with layered screens and original artwork.
class _DevicesPainter extends CustomPainter {
  const _DevicesPainter();
  static const ink = Color(0xff20262e);
  @override
  void paint(Canvas c, Size size) {
    c.save();
    c.scale(size.width / 640, size.height / 400);
    void rect(double x, double y, double w, double h, Color color,
        [double r = 3]) {
      c.drawRRect(
          RRect.fromRectAndRadius(
              Rect.fromLTWH(x, y, w, h), Radius.circular(r)),
          Paint()..color = color);
    }

    void label(String text, double x, double y, double font, Color color,
        [FontWeight weight = FontWeight.w400]) {
      final p = TextPainter(
          text: TextSpan(
              text: text,
              style: TextStyle(
                  fontSize: font,
                  color: color,
                  fontWeight: weight,
                  fontFamily: 'Segoe UI')),
          textDirection: TextDirection.ltr)
        ..layout();
      p.paint(c, Offset(x, y));
    }

    void shadow(Rect bounds, double radius, double elevation) {
      c.drawShadow(
          Path()
            ..addRRect(
                RRect.fromRectAndRadius(bounds, Radius.circular(radius))),
          const Color(0xff405273),
          elevation,
          true);
    }

    void wallpaper(Rect bounds) {
      c.save();
      c.clipRRect(RRect.fromRectAndRadius(bounds, const Radius.circular(3)));
      c.drawRect(
          bounds,
          Paint()
            ..shader = const LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  Color(0xffbadfec),
                  Color(0xff71b7ee),
                  Color(0xff4d85e5)
                ]).createShader(bounds));
      c.translate(bounds.left, bounds.top);
      c.scale(bounds.width / 280, bounds.height / 170);
      // Overlapping luminous folds give the screen depth without bitmap scaling.
      for (var i = 0; i < 9; i++) {
        final d = i * 9.0;
        final fold = Path()
          ..moveTo(35 + d, 190)
          ..cubicTo(-10 + d, 97, 170 - d, -40, 208 + d, 45)
          ..cubicTo(260 + d, 113, 80 + d, 93, 126 + d, 195)
          ..close();
        c.drawPath(
            fold,
            Paint()
              ..shader = LinearGradient(
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                  colors: [
                    Color.lerp(const Color(0xff6de4ff), const Color(0xff2579f5),
                        i / 10)!,
                    const Color(0xff1850d0),
                    const Color(0xff06247b)
                  ]).createShader(Rect.fromLTWH(25 + d, 15, 195, 180)));
        final edge = Path()
          ..moveTo(35 + d, 190)
          ..cubicTo(-10 + d, 97, 170 - d, -40, 208 + d, 45);
        c.drawPath(
            edge,
            Paint()
              ..color = const Color(0x888bdfff)
              ..style = PaintingStyle.stroke
              ..strokeWidth = 1);
      }
      c.restore();
    }

    void app(double x, double y, int i, [double scale = 1]) {
      final colors = [
        const Color(0xff3c85ff),
        const Color(0xff19b8a2),
        const Color(0xfff3aa38),
        const Color(0xff8662e6),
        const Color(0xffec657b),
        const Color(0xff36a7d8)
      ];
      c.save();
      c.translate(x, y);
      c.scale(scale);
      final b = const Rect.fromLTWH(0, 0, 18, 18);
      c.drawRRect(
          RRect.fromRectAndRadius(b, const Radius.circular(5)),
          Paint()
            ..shader = LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  Color.lerp(colors[i % 6], Colors.white, .2)!,
                  colors[i % 6]
                ]).createShader(b));
      final icon = [
        Icons.folder_rounded,
        Icons.headphones_rounded,
        Icons.sports_esports_rounded,
        Icons.image_rounded,
        Icons.play_arrow_rounded,
        Icons.language_rounded
      ][i % 6];
      final p = TextPainter(
          text: TextSpan(
              text: String.fromCharCode(icon.codePoint),
              style: TextStyle(
                  fontFamily: icon.fontFamily,
                  fontSize: 12,
                  color: Colors.white)),
          textDirection: TextDirection.ltr)
        ..layout();
      p.paint(c, const Offset(3, 3));
      c.restore();
    }

    // Tablet housing, subtle edge highlight, and its miniature application.
    const tablet = Rect.fromLTWH(75, 73, 444, 284);
    shadow(tablet, 10, 3);
    rect(75, 73, 444, 284, const Color(0xff24282d), 10);
    c.drawRRect(
        RRect.fromRectAndRadius(tablet, const Radius.circular(10)),
        Paint()
          ..color = const Color(0xff45494e)
          ..style = PaintingStyle.stroke
          ..strokeWidth = .8);
    rect(83, 82, 428, 266, const Color(0xfff8fbfd), 3);
    rect(83, 82, 428, 20, const Color(0xffeef3f6), 2);
    label('OpenUU', 99, 88, 5, ink, FontWeight.w600);
    rect(90, 89, 5, 5, const Color(0xff4286ff), 1);
    for (var i = 0; i < 4; i++)
      c.drawCircle(Offset(453 + i * 13, 92), 2,
          Paint()..color = const Color(0xff92a0aa));
    rect(83, 102, 96, 246, const Color(0xffeff4f7), 0);
    label('MY DEVICES', 92, 116, 5, const Color(0xff7f8d9b), FontWeight.w600);
    for (var i = 0; i < 4; i++) {
      if (i == 0) rect(88, 130, 86, 18, const Color(0xffdfeaff), 3);
      rect(95, 136 + i * 22, 6, 6,
          i == 0 ? const Color(0xff4286ff) : const Color(0xffa8b7c4), 2);
      label(
          [
            'Home computer',
            'Office workstation',
            'All devices',
            'Remote assistance'
          ][i],
          106,
          136 + i * 22,
          5.3,
          ink);
    }
    label('Settings', 101, 333, 5.3, const Color(0xff83919e));
    rect(191, 113, 24, 9, const Color(0xffd9f4e5), 4);
    label('ONLINE', 195, 115, 4, const Color(0xff289465), FontWeight.w600);
    label('Home computer', 222, 112, 8, ink, FontWeight.w600);
    wallpaper(const Rect.fromLTWH(191, 132, 307, 151));
    rect(191, 283, 307, 19, const Color(0xffeef3f7), 0);
    label('Files          Remote control          Display          More', 205,
        289, 5.3, const Color(0xff718191));
    label('Quick access', 193, 309, 5.5, const Color(0xff566676),
        FontWeight.w600);
    for (var i = 0; i < 6; i++) {
      app(197 + i * 49, 321, i);
      label(['Files', 'Music', 'Games', 'Photos', 'Video', 'Browser'][i],
          195 + i * 49, 341, 4.1, const Color(0xff83919e));
    }
    // Phone on the same baseline, overlapping the tablet rather than floating.
    const phone = Rect.fromLTWH(483, 117, 125, 244);
    shadow(phone, 26, 3);
    rect(483, 117, 125, 244, const Color(0xff262b30), 26);
    rect(489, 123, 113, 232, const Color(0xfff5f9fc), 21);
    rect(526, 126, 39, 8, const Color(0xff262b30), 6);
    label('9:41', 498, 129, 4.5, ink, FontWeight.w600);
    label('Home computer', 498, 147, 6, ink, FontWeight.w600);
    wallpaper(const Rect.fromLTWH(496, 162, 99, 105));
    label('Control     Files     More', 501, 276, 4.5, const Color(0xff6d7d8e));
    label('Quick access', 498, 292, 4.7, ink);
    for (var i = 0; i < 6; i++)
      app(499 + (i % 3) * 33, 304 + (i ~/ 3) * 23, i, .73);
    rect(531, 348, 30, 2, ink, 2);
    void badge(String text, double x, double y, Color color, double angle) {
      c.save();
      c.translate(x, y);
      c.rotate(angle);
      shadow(const Rect.fromLTWH(0, 0, 39, 25), 5, 2);
      rect(0, 0, 39, 25, color, 5);
      final tail = Path()
        ..moveTo(24, 24)
        ..lineTo(30, 30)
        ..lineTo(30, 24)
        ..close();
      c.drawPath(tail, Paint()..color = color);
      label(text, 7, 5, 12, Colors.white, FontWeight.w600);
      c.restore();
    }

    badge('4K', 21, 170, const Color(0xffffa024), -.32);
    badge('Free', 558, 48, const Color(0xff18bb60), .43);
    final bolt = Path()
      ..moveTo(370, 18)
      ..lineTo(347, 38)
      ..lineTo(359, 41)
      ..lineTo(354, 57)
      ..lineTo(377, 33)
      ..lineTo(365, 31)
      ..close();
    c.drawPath(bolt, Paint()..color = const Color(0xffffd52f));
    c.save();
    c.translate(181, 38);
    c.rotate(-.45);
    rect(0, 10, 8, 15, const Color(0xff327fff), 3);
    rect(10, 0, 13, 22, const Color(0xff1666ec), 4);
    c.restore();
    final star = Path()
      ..moveTo(630, 186)
      ..quadraticBezierTo(630, 194, 622, 195)
      ..quadraticBezierTo(630, 196, 631, 204)
      ..quadraticBezierTo(632, 196, 639, 195)
      ..quadraticBezierTo(631, 194, 630, 186)
      ..close();
    c.drawPath(star, Paint()..color = const Color(0xff8d3bff));
    c.restore();
  }

  @override
  bool shouldRepaint(covariant _DevicesPainter oldDelegate) => false;
}
