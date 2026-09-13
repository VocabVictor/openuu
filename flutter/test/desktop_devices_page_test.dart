import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/pages/desktop_devices_page.dart';
import 'package:flutter_hbb/models/peer_model.dart';

void main() {
  for (final width in [420.0, 1000.0]) {
    testWidgets('grouped devices at $width', (tester) async {
      String? opened;
      final peers = [
        Peer.fromJson({'id': '1', 'hostname': 'Local PC', 'platform': 'Windows'}),
        Peer.fromJson({'id': '2', 'hostname': 'Remote PC', 'platform': 'Linux'}),
        Peer.fromJson({'id': '3', 'hostname': 'Phone', 'platform': 'Android'}),
      ];
      await tester.pumpWidget(MaterialApp(home: Scaffold(body: SizedBox(width: width,
        child: DeviceGroups(peers: peers, localId: '1', onOpen: (p) => opened = p.id)))));
      expect(find.text('Computers'), findsOneWidget);
      expect(find.text('Phones / tablets'), findsOneWidget);
      expect(find.text('This device'), findsOneWidget);
      await tester.tap(find.text('Local PC'));
      expect(opened, isNull);
      await tester.tap(find.text('Remote PC'));
      expect(opened, '2');
      await tester.tap(find.text('Computers'));
      await tester.pumpAndSettle();
      expect(find.text('Remote PC'), findsNothing);
      expect(find.text('Phone'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('the action icons keep one column whatever the name', (tester) async {
    final peers = [
      Peer.fromJson({'id': '1', 'hostname': 'This one', 'platform': 'Windows'}),
      Peer.fromJson({'id': '2', 'hostname': 'PC', 'platform': 'Windows'}),
      Peer.fromJson({
        'id': '3',
        'hostname': 'a-really-quite-long-machine-name',
        'platform': 'Windows'
      }),
    ];
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: SizedBox(
                width: 900,
                child: DeviceGroups(
                    peers: peers,
                    localId: '1',
                    favorites: {'2'},
                    onToggleFavorite: (_) {},
                    onOpen: (_) {})))));
    // Every row that has them puts its star and its chevron on one column;
    // identical icons cannot be told apart by widget, so measure the render
    // boxes of the matching elements.
    Set<double> centresOf(bool Function(Icon) matches) => find
        .byWidgetPredicate((w) => w is Icon && matches(w))
        .evaluate()
        .map((e) {
          final box = e.renderObject as RenderBox;
          return box.localToGlobal(box.size.center(Offset.zero)).dx;
        })
        .toSet();
    final starX =
        centresOf((i) => i.icon == Icons.star || i.icon == Icons.star_border);
    expect(starX.length, 1, reason: 'stars drifted with the name length');
    final chevronX = centresOf((i) => i.icon == Icons.chevron_right);
    expect(chevronX.length, 1, reason: 'chevrons drifted with the name length');
    expect(chevronX.first, greaterThan(starX.first));
    expect(tester.takeException(), isNull);
  });
}
