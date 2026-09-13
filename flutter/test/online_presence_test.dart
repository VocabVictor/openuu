import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/online_presence.dart';

void main() {
  final t0 = DateTime(2026, 9, 13, 12);
  const interval = Duration(seconds: 6);

  group('when a query is due', () {
    test('the first query for a list goes at once', () {
      final s = OnlinePollSchedule(interval: interval);
      expect(s.due(ids: {'1'}, visible: true, now: t0), isTrue);
    });

    test('nothing is asked while the interval runs', () {
      final s = OnlinePollSchedule(interval: interval);
      s.sent({'1'}, t0);
      expect(s.due(ids: {'1'}, visible: true, now: t0), isFalse);
      expect(
          s.due(
              ids: {'1'},
              visible: true,
              now: t0.add(const Duration(seconds: 5, milliseconds: 999))),
          isFalse);
      expect(s.due(ids: {'1'}, visible: true, now: t0.add(interval)), isTrue);
    });

    test('a device that was not in the last question is asked about now', () {
      final s = OnlinePollSchedule(interval: interval);
      s.sent({'1'}, t0);
      expect(s.due(ids: {'1', '2'}, visible: true, now: t0), isTrue,
          reason: 'a new device must not wait out the interval');
    });

    test('a device that went away is a new question too', () {
      final s = OnlinePollSchedule(interval: interval);
      s.sent({'1', '2'}, t0);
      expect(s.due(ids: {'1'}, visible: true, now: t0), isTrue);
    });

    test('an empty list asks nothing, ever', () {
      final s = OnlinePollSchedule(interval: interval);
      expect(
          s.due(
              ids: const {},
              visible: true,
              now: t0.add(const Duration(hours: 1))),
          isFalse);
    });

    test('a page nobody is looking at asks nothing', () {
      final s = OnlinePollSchedule(interval: interval);
      s.sent({'1'}, t0);
      expect(
          s.due(ids: {'1'}, visible: false, now: t0.add(const Duration(days: 1))),
          isFalse,
          reason: 'the poll must not spin in the background');
      expect(s.due(ids: {'1'}, visible: true, now: t0.add(interval)), isTrue,
          reason: 'and it resumes when the page comes back');
    });
  });

  group('what is known about a peer', () {
    test('a peer nobody asked about is unknown, not offline', () {
      final book = PresenceBook();
      expect(book.of('1'), PeerPresence.unknown);
      expect(book.isEmpty, isTrue);
    });

    test('the two lists of one reply are both answers', () {
      final book = PresenceBook();
      book.record(onlines: ['1'], offlines: ['2']);
      expect(book.of('1'), PeerPresence.online);
      expect(book.of('2'), PeerPresence.offline);
      expect(book.of('3'), PeerPresence.unknown,
          reason: 'this reply said nothing about 3');
    });

    test('a later reply overrides an earlier one', () {
      final book = PresenceBook();
      book.record(onlines: ['1']);
      expect(book.of('1'), PeerPresence.online);
      book.record(offlines: ['1']);
      expect(book.of('1'), PeerPresence.offline);
      book.record(onlines: ['1']);
      expect(book.of('1'), PeerPresence.online);
    });

    test('what was known goes stale together', () {
      final book = PresenceBook();
      book.record(onlines: ['1'], offlines: ['2']);
      book.clear();
      expect(book.of('1'), PeerPresence.unknown);
      expect(book.of('2'), PeerPresence.unknown);
    });
  });

  group('the reply lists', () {
    test('a comma separated list becomes ids', () {
      expect(parsePeerIdList('1,2,3'), ['1', '2', '3']);
    });

    test('an empty or missing list is no ids', () {
      expect(parsePeerIdList(''), isEmpty);
      expect(parsePeerIdList(null), isEmpty);
      expect(parsePeerIdList(42), isEmpty);
      expect(parsePeerIdList(',,'), isEmpty);
    });

    test('spacing around an id does not make a different id', () {
      expect(parsePeerIdList(' 1 , 2 '), ['1', '2']);
    });
  });
}
