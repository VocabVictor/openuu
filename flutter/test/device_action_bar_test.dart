import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/widgets/device_action_bar.dart';

void main() {
  for (final width in [320.0, 600.0, 1000.0]) {
    testWidgets('toolbar fits $width and dispatches actions', (tester) async {
      final calls = <int>[];
      await tester.pumpWidget(MaterialApp(home: Scaffold(body: Center(
        child: SizedBox(width: width, child: DeviceActionBar(id: '123456789',
          onFiles: () => calls.add(0), onWatch: () => calls.add(1),
          onTerminal: () => calls.add(2), onTunnel: () => calls.add(3))),
      ))));
      expect(tester.takeException(), isNull);
      final buttons = find.byType(TextButton);
      expect(buttons, findsNWidgets(4));
      expect(find.byType(VerticalDivider), findsNWidgets(4));
      final first = tester.getRect(buttons.first);
      for (var i = 0; i < 4; i++) {
        final rect = tester.getRect(buttons.at(i));
        expect(rect.width, closeTo(first.width, 0.1));
        expect(rect.top, first.top);
        await tester.tap(buttons.at(i));
      }
      expect(calls, [0, 1, 2, 3]);
      String? copied;
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform, (call) async {
          if (call.method == 'Clipboard.setData') copied = call.arguments['text'];
          return null;
        });
      addTearDown(() => tester.binding.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, null));
      await tester.tap(find.byTooltip('More'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Copy device ID'));
      await tester.pumpAndSettle();
      expect(copied, '123456789');
    });
  }
}
