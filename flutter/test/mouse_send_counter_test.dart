import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_model/send_counter.dart';

void main() {
  test('nothing is reported before a second has passed', () {
    final c = MouseSendCounter();
    for (var t = 0; t < 1000000; t += 1000) {
      expect(c.record('move', t), isNull);
    }
  });

  test('the first message of a new second reports the one before it', () {
    final c = MouseSendCounter();
    for (var t = 0; t < 1000000; t += 8000) {
      c.record('move', t);
    }
    final report = c.record('move', 1000000);
    expect(report, isNotNull);
    expect(report!.total, 125);
    expect(report.perSecond, closeTo(125, 0.1));
  });

  test('the counts are kept apart by type', () {
    final c = MouseSendCounter();
    c.record('down', 0);
    for (var t = 1000; t < 1000000; t += 1000) {
      c.record('move', t);
    }
    c.record('up', 1000000 - 1);
    final report = c.record('move', 1000000)!;
    expect(report.counts['down'], 1);
    expect(report.counts['up'], 1);
    expect(report.counts['move'], 999);
    expect(report.toString(), contains('down=1'));
  });

  test('a message after a long idle counts in the next second, not the last',
      () {
    final c = MouseSendCounter();
    c.record('move', 0);
    final report = c.record('move', 5000000)!;
    expect(report.total, 1, reason: 'only the message that started the span');
    expect(report.spanUs, 5000000);
    expect(report.perSecond, closeTo(0.2, 0.01));
  });
}
