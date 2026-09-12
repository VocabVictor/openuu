import 'package:flutter/material.dart';
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
      await tester.tap(find.byTooltip('More'));
      await tester.pumpAndSettle();
      expect(find.text('More tools'), findsOneWidget);
      expect(tester.widget<ListTile>(find.byKey(const ValueKey('tool-4'))).enabled, isFalse);
      await tester.tap(find.text('Reorder'));
      await tester.pumpAndSettle();
      await tester.tap(find.descendant(of: find.byKey(const ValueKey('tool-1')),
        matching: find.byTooltip('Move up')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Done'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('tool-1')));
      await tester.pumpAndSettle();
      expect(calls, [0, 1, 2, 3, 1]);
      expect(find.text('More tools'), findsNothing);
      expect(tester.getTopLeft(find.byTooltip('View only')).dx,
        lessThan(tester.getTopLeft(find.byTooltip('Files')).dx));
      expect(tester.takeException(), isNull);
    });
  }
}
