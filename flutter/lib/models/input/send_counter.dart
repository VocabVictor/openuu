/// Counts the mouse messages this side actually puts on the wire.
///
/// The point of coalescing pointer moves is a smaller number here, and there
/// was no way to observe that number: no log line carries it, and on the wire
/// the messages are encrypted, so a capture can only weigh bytes. The counter
/// lands before the coalescing does, so a build without coalescing and a
/// build with it report the same quantity.
///
/// Off unless RUSTDESK_INPUT_VERBOSE=1 is set.
library;

const int _kReportUs = 1000000;

/// One second's worth of counts, by message type.
class MouseSendReport {
  const MouseSendReport(this.counts, this.spanUs);

  final Map<String, int> counts;
  final int spanUs;

  int get total => counts.values.fold(0, (a, b) => a + b);

  double get perSecond => spanUs == 0 ? 0 : total * 1000000 / spanUs;

  @override
  String toString() {
    final parts = counts.keys.toList()..sort();
    final detail = parts.map((k) => '$k=${counts[k]}').join(' ');
    return 'mouse messages sent: ${perSecond.toStringAsFixed(1)}/s '
        '(total=$total span=${(spanUs / 1000).round()}ms $detail)';
  }
}

class MouseSendCounter {
  MouseSendCounter({this.reportUs = _kReportUs});

  final int reportUs;

  final Map<String, int> _counts = {};
  int? _sinceUs;

  /// Records one message. Returns a report on the first message of a new
  /// second, so the caller needs no timer of its own.
  MouseSendReport? record(String type, int nowUs) {
    final since = _sinceUs;
    if (since == null) {
      _sinceUs = nowUs;
      _counts[type] = 1;
      return null;
    }
    MouseSendReport? out;
    if (nowUs - since >= reportUs) {
      out = MouseSendReport(Map.unmodifiable(_counts), nowUs - since);
      _counts.clear();
      _sinceUs = nowUs;
    }
    _counts.update(type, (v) => v + 1, ifAbsent: () => 1);
    return out;
  }
}
