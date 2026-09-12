import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/pages/desktop_device_page.dart';

void main() {
  for (final size in [const Size(800, 500), const Size(1280, 720), const Size(1920, 1080), const Size(1600, 600)]) {
    testWidgets('device details fit $size and dispatch connection', (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = size;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      var connections = 0;
      await tester.pumpWidget(MaterialApp(home: DesktopDevicePage(
        name: 'DESKTOP-A-VERY-LONG-DEVICE-NAME-123456789', id: '123456789', online: true,
        onBack: () {}, onLogin: () {}, onSettings: () {}, onAssistance: () {}, onFavorites: () {},
        onWatch: () => connections++, onConnect: () => connections++, onFiles: () {}, onTerminal: () {}, onTunnel: () {})));
      expect(tester.takeException(), isNull);
      await tester.tap(find.text('Enter desktop  →'));
      expect(connections, 1);
      final viewOnly = tester.widget<TextButton>(find.descendant(of: find.byTooltip('View only'), matching: find.byType(TextButton)).first);
      expect(viewOnly.onPressed, isNotNull);
      viewOnly.onPressed!();
      expect(connections, 2);
    });
  }
}
