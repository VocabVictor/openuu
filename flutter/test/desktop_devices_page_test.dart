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
      expect(find.text('Computers 2'), findsOneWidget);
      expect(find.text('Phones / tablets 1'), findsOneWidget);
      expect(find.text('This device'), findsOneWidget);
      await tester.tap(find.text('Local PC'));
      expect(opened, isNull);
      await tester.tap(find.text('Remote PC'));
      expect(opened, '2');
      await tester.tap(find.text('Computers 2'));
      await tester.pumpAndSettle();
      expect(find.text('Remote PC'), findsNothing);
      expect(find.text('Phone'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  }
}
