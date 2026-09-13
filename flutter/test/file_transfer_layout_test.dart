import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/widgets/file_transfer_layout.dart';

void main() {
  for (final size in [
    const Size(800, 500),
    const Size(1280, 720),
    const Size(1920, 1080)
  ]) {
    for (final dpi in [1.0, 1.5, 2.0]) {
      testWidgets('file transfer layout $size DPI $dpi', (tester) async {
        tester.view.devicePixelRatio = dpi;
        tester.view.physicalSize = size * dpi;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        var sends = 0;
        await tester.pumpWidget(MaterialApp(
            home: FileTransferLayout(
                localName: 'LOCAL-DESKTOP-WITH-LONG-NAME',
                remoteName: 'REMOTE-DESKTOP-WITH-LONG-NAME',
                localBrowser: const SizedBox.expand(key: ValueKey('local')),
                remoteBrowser: const SizedBox.expand(key: ValueKey('remote')),
                transfers: const SizedBox.expand(key: ValueKey('queue')),
                onSend: () => sends++)));
        expect(tester.takeException(), isNull);
        final local = tester.getRect(find.byKey(const ValueKey('local')));
        final remote = tester.getRect(find.byKey(const ValueKey('remote')));
        final queue = tester.getRect(find.byKey(const ValueKey('queue')));
        expect(local.width, closeTo(remote.width, .01));
        expect(local.right, lessThan(remote.left));
        expect(queue.top, greaterThan(local.bottom));
        expect(queue.bottom, lessThanOrEqualTo(size.height));
        // The two buttons no longer share a label: the right-hand one is
        // Receive, and it is disabled here because onReceive was not given.
        expect(find.text('Send'), findsOneWidget);
        expect(find.text('Receive'), findsOneWidget);
        await tester.tap(find.text('Send'));
        expect(sends, 1);
        await tester.tap(find.text('Receive'));
        expect(sends, 1);
      });
    }
  }
}
