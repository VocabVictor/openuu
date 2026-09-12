import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/quick_launch_model.dart';

void main() {
  test('correlates concurrent remote responses', () async {
    final sent = <Map<String, dynamic>>[];
    Future<void> transport(String raw) async { sent.add(jsonDecode(raw)); }
    final first = QuickLaunchRequests.send(transport, {'operation': 'list'});
    final second = QuickLaunchRequests.send(transport, {'operation': 'launch'});
    await Future<void>.delayed(Duration.zero);
    QuickLaunchRequests.receive(jsonEncode({'request_id': sent[1]['request_id'], 'data': {'launched': true}}));
    QuickLaunchRequests.receive(jsonEncode({'request_id': sent[0]['request_id'], 'data': {'apps': []}}));
    expect((await first)['apps'], isEmpty);
    expect((await second)['launched'], isTrue);
  });
  test('ignores a response from another session', () async {
    String? id;
    var completed = false;
    final future = QuickLaunchRequests.send((raw) async { id = jsonDecode(raw)['request_id']; },
      {'operation': 'list'}, scope: 'session-a').then((value) { completed = true; return value; });
    await Future<void>.delayed(Duration.zero);
    final response = jsonEncode({'request_id': id, 'data': {'apps': []}});
    QuickLaunchRequests.receive(response, scope: 'session-b');
    await Future<void>.delayed(Duration.zero);
    expect(completed, isFalse);
    QuickLaunchRequests.receive(response, scope: 'session-a');
    expect((await future)['apps'], isEmpty);
  });
  test('surfaces remote denial without treating it as a launch', () async {
    String? requestId;
    final result = QuickLaunchRequests.send((raw) async { requestId = jsonDecode(raw)['request_id']; }, {'operation': 'launch'});
    final expectation = expectLater(result, throwsStateError);
    await Future<void>.delayed(Duration.zero);
    QuickLaunchRequests.receive(jsonEncode({'request_id': requestId, 'error': 'Permission denied'}));
    await expectation;
  });
}
