import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/widgets/desktop_preview.dart';

void main() {
  test('preview keys isolate login, server and device', () {
    final key = DesktopPreview.keyFor('server', 'session', 'peer');
    expect(key, isNotNull);
    expect(DesktopPreview.keyFor('server', '', 'peer'), isNull);
    expect(DesktopPreview.keyFor('server', 'session', ''), isNull);
    expect(DesktopPreview.keyFor('other', 'session', 'peer'), isNot(key));
    expect(DesktopPreview.keyFor('server', 'other', 'peer'), isNot(key));
    expect(DesktopPreview.keyFor('server', 'session', 'other'), isNot(key));
  });

  testWidgets('empty preview connects and reloads without another connection',
      (tester) async {
    var connections = 0;
    var reads = 0;
    await tester.pumpWidget(MaterialApp(
        home: Center(
            child: SizedBox(
                width: 640,
                child: DesktopPreviewPanel(
                    peer: 'test',
                    onConnect: () => connections++,
                    loadPreview: () async {
                      reads++;
                      return null;
                    })))));
    await tester.pump();
    expect(find.text('A preview will be saved after connecting'), findsOneWidget);
    final size = tester.getSize(find.byType(AspectRatio));
    expect(size.width / size.height, closeTo(16 / 9, .001));
    await tester.tap(find.text('Enter desktop  →'));
    expect(connections, 1);
    await tester.tap(find.byTooltip('Reload saved preview'));
    await tester.pump();
    expect(reads, 2);
    expect(connections, 1);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 35));
    expect(reads, 2);
  });

  testWidgets('saved image is contained and explicitly not live',
      (tester) async {
    final recorder = ui.PictureRecorder();
    Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
    final picture = recorder.endRecording();
    final data = await tester.runAsync(() async {
      final image = await picture.toImage(16, 9);
      final data = await image.toByteData(format: ui.ImageByteFormat.png);
      image.dispose();
      picture.dispose();
      return data;
    });
    await tester.pumpWidget(MaterialApp(
        home: Center(
            child: SizedBox(
                width: 640,
                child: DesktopPreviewPanel(
                    peer: 'test',
                    onConnect: () {},
                    loadPreview: () async => DesktopPreview(
                        data!.buffer.asUint8List(),
                        DateTime(2026, 9, 12, 10, 30)))))));
    await tester.pump();
    expect(find.textContaining('Not live'), findsOneWidget);
    expect(tester.widget<Image>(find.byType(Image)).fit, BoxFit.contain);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('read failure retains connect action', (tester) async {
    await tester.pumpWidget(MaterialApp(
        home: DesktopPreviewPanel(
            peer: 'test',
            onConnect: () {},
            loadPreview: () async => throw StateError('test failure'))));
    await tester.pump();
    expect(find.text('Unable to load preview'), findsOneWidget);
    expect(find.text('Enter desktop  →'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
}
