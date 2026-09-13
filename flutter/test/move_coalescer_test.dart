import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_model/move_coalescer.dart';

void main() {
  const us = kMoveCoalesceIntervalUs;

  test('the first move goes straight out', () {
    final c = MoveCoalescer<String>();
    expect(c.offerMove('a', 0), ['a']);
    expect(c.hasPending, isFalse);
  });

  test('moves inside one interval collapse to the last position', () {
    final c = MoveCoalescer<String>();
    c.offerMove('a', 0);
    expect(c.offerMove('b', 1000), isEmpty);
    expect(c.offerMove('c', 2000), isEmpty);
    expect(c.offerMove('d', 3000), isEmpty);
    expect(c.pending, 'd', reason: 'the newest position is the one worth sending');
    expect(c.flush(4000), ['d']);
    expect(c.hasPending, isFalse);
  });

  test('a move past the interval boundary is sent without waiting', () {
    final c = MoveCoalescer<String>();
    c.offerMove('a', 0);
    c.offerMove('b', 1000);
    expect(c.offerMove('c', us), ['c']);
    expect(c.hasPending, isFalse, reason: 'b is superseded, not sent late');
  });

  test('a press during movement sends the last position first', () {
    final c = MoveCoalescer<String>();
    c.offerMove('a', 0);
    c.offerMove('b', 1000);
    expect(c.offerImmediate('down', 2000), ['b', 'down'],
        reason: 'the press must land where the user pressed');
  });

  test('a press with nothing held is sent alone', () {
    final c = MoveCoalescer<String>();
    expect(c.offerImmediate('down', 0), ['down']);
  });

  test('presses are never held back, however fast they come', () {
    final c = MoveCoalescer<String>();
    for (var t = 0; t < 10; t++) {
      expect(c.offerImmediate('click$t', t * 100), ['click$t']);
    }
  });

  test('a pointer that stops leaves nothing unsent', () {
    final c = MoveCoalescer<String>();
    c.offerMove('a', 0);
    c.offerMove('b', 1000);
    expect(c.flush(1500), ['b']);
    expect(c.flush(2000), isEmpty, reason: 'flushing twice sends nothing twice');
  });

  test('the wait is what is left of the interval', () {
    final c = MoveCoalescer<String>();
    expect(c.delayUs(0), isNull);
    c.offerMove('a', 0);
    expect(c.delayUs(0), isNull, reason: 'nothing is held');
    c.offerMove('b', 1000);
    expect(c.delayUs(1000), us - 1000);
    expect(c.delayUs(us + 500), 0, reason: 'already due');
  });

  test('a mouse reporting at 1000 Hz is cut to the interval rate', () {
    final c = MoveCoalescer<String>();
    var sent = 0;
    for (var t = 0; t < 1000000; t += 1000) {
      sent += c.offerMove('m$t', t).length;
    }
    expect(sent, 125, reason: 'one message per interval, not per report');
  });

  test('a drag sends every press and release plus bounded moves', () {
    final c = MoveCoalescer<String>();
    final sent = <String>[];
    sent.addAll(c.offerImmediate('down', 0));
    for (var t = 1000; t < 100000; t += 1000) {
      sent.addAll(c.offerMove('m$t', t));
    }
    sent.addAll(c.offerImmediate('up', 100000));
    expect(sent.first, 'down');
    expect(sent.last, 'up');
    expect(sent.where((e) => e.startsWith('m')).length, lessThan(20),
        reason: '99 reports, at most one per 8 ms');
    expect(sent[sent.length - 2], 'm99000',
        reason: 'the release follows the last position the pointer reached');
  });

  test('reset drops what is held', () {
    final c = MoveCoalescer<String>();
    c.offerMove('a', 0);
    c.offerMove('b', 1000);
    c.reset();
    expect(c.hasPending, isFalse);
    expect(c.flush(2000), isEmpty);
    expect(c.offerMove('c', 2000), ['c'], reason: 'the clock starts again');
  });
}
