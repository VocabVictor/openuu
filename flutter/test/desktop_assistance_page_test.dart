import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/pages/desktop_assistance_page.dart';

void main() {
  for (final size in [
    const Size(960, 600),
    const Size(1280, 720),
    const Size(1920, 1080)
  ]) {
    testWidgets('Assistance cards fit $size and connect with entered ID',
        (tester) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      String? connected;
      await tester.pumpWidget(MaterialApp(
          home: DesktopAssistancePage(
              deviceId: '123 456 789',
              password: 'test-only',
              verification: 'Temporary password',
              verificationMethod: 'use-temporary-password',
              onVerificationChanged: (_) {},
              enabled: true,
              temporaryPassword: true,
              onEnable: (_) async {},
              onConnect: (id) => connected = id,
              onRefresh: () {},
              onSecurity: () {},
              onDevices: () {},
              onFavorites: () {},
              onSettings: () {},
              onAccount: () {})));
      expect(tester.takeException(), isNull);
      expect(find.text('test-only'), findsNothing);
      final connect = find.widgetWithText(ElevatedButton, 'Connect');
      expect(tester.widget<ElevatedButton>(connect).onPressed, isNull);
      await tester.enterText(find.byType(TextField), '987654321');
      await tester.ensureVisible(connect);
      await tester.pump();
      await tester.tap(connect);
      expect(connected, '987654321');
      expect(tester.takeException(), isNull);
    });
  }
}
