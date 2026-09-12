import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/pages/desktop_welcome_page.dart';

void main() {
  for (final size in [const Size(800, 500), const Size(1280, 720),
    const Size(1920, 1080), const Size(1600, 600), const Size(2560, 1080)]) {
    for (final dpi in [1.0, 1.5, 2.0]) {
      testWidgets('landscape $size at $dpi DPI scale', (tester) async {
        tester.view.devicePixelRatio = dpi;
        tester.view.physicalSize = size * dpi;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        var logins = 0;
        await tester.pumpWidget(MaterialApp(home: DesktopWelcomePage(
          onLogin: () => logins++, onAssistance: () {}, onFavorites: () {},
          onSettings: () {})));
        expect(tester.takeException(), isNull);
        final login = find.byKey(const ValueKey('welcome-login'));
        final bounds = tester.getRect(login);
        expect(bounds.bottom, lessThanOrEqualTo(size.height));
        expect(bounds.top, greaterThanOrEqualTo(0));
        final art = tester.getSize(find.byKey(const ValueKey('welcome-artwork')));
        expect(art.width / art.height, closeTo(1.6, .001));
        await tester.tap(login);
        expect(logins, 1);
      });
    }
  }
}
