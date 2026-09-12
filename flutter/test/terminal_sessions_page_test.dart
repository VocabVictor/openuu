import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/pages/terminal_sessions_page.dart';

void main() {
  for (final size in [const Size(800, 500), const Size(1280, 720), const Size(1920, 1080)]) {
    testWidgets('session manager fits $size and dispatches actions', (tester) async {
      tester.view.devicePixelRatio = 1.5;
      tester.view.physicalSize = size * 1.5;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      var created = 0;
      String? opened;
      await tester.pumpWidget(MaterialApp(home: TerminalSessionsPage(
        device: 'DESKTOP-LONG-NAME-12345678901234567890',
        sessions: const [TerminalSessionEntry(key: 'one', name: 'session1', connected: true),
          TerminalSessionEntry(key: 'two', name: 'session2', connected: false)],
        onCreate: () => created++, onOpen: (key) => opened = key,
        onRemove: (_) {}, onDrag: () {}, onMinimize: () {}, onClose: () {})));
      expect(tester.takeException(), isNull);
      await tester.tap(find.text('Create terminal session'));
      expect(created, 1);
      await tester.tap(find.text('Open terminal').first);
      expect(opened, 'one');
      final pendingButton = tester.widget<OutlinedButton>(find.ancestor(
        of: find.text('Open terminal').last, matching: find.byType(OutlinedButton)));
      expect(pendingButton.onPressed, isNull);
    });
  }
}
