/// Which pointer messages actually go on the wire.
///
/// Flutter delivers a pointer move for every report the mouse sends, up to a
/// thousand a second on a gaming mouse, and every one of them used to become
/// its own message to the peer (docs/perf-review.md P1-6). Only the last
/// position of an interval is worth sending: the ones before it are already
/// obsolete when they arrive.
///
/// Presses, releases, wheel and drag boundaries are never held back. A press
/// is only meaningful at the position it happened at, and a press message
/// carries no position of its own, so a held move is sent first.
library;

/// About one message per 120 Hz frame. Short enough that the remote cursor
/// still looks continuous, long enough to collapse a high-rate mouse.
const int kMoveCoalesceIntervalUs = 8000;

class MoveCoalescer<T> {
  MoveCoalescer({this.intervalUs = kMoveCoalesceIntervalUs});

  final int intervalUs;

  T? _pending;
  int? _lastSentUs;

  /// The move being held back, if any.
  T? get pending => _pending;

  bool get hasPending => _pending != null;

  /// When the held move may be sent, or null when nothing is held.
  int? dueUs() {
    final last = _lastSentUs;
    if (_pending == null || last == null) {
      return _pending == null ? null : 0;
    }
    return last + intervalUs;
  }

  /// How long from [nowUs] until the held move may be sent; zero when it is
  /// due already, null when nothing is held.
  int? delayUs(int nowUs) {
    final due = dueUs();
    if (due == null) {
      return null;
    }
    final left = due - nowUs;
    return left > 0 ? left : 0;
  }

  /// A pointer move. Returns what to send now: the move itself when the
  /// interval has elapsed, nothing when it is held for later.
  List<T> offerMove(T evt, int nowUs) {
    final last = _lastSentUs;
    if (last == null || nowUs - last >= intervalUs) {
      _pending = null;
      _lastSentUs = nowUs;
      return [evt];
    }
    _pending = evt;
    return const [];
  }

  /// An event that must not be delayed or dropped. The held move goes out
  /// ahead of it so the peer acts on it at the right position.
  List<T> offerImmediate(T evt, int nowUs) {
    final held = _pending;
    _pending = null;
    if (held == null) {
      return [evt];
    }
    _lastSentUs = nowUs;
    return [held, evt];
  }

  /// The interval boundary, or a pointer that stopped moving.
  List<T> flush(int nowUs) {
    final held = _pending;
    if (held == null) {
      return const [];
    }
    _pending = null;
    _lastSentUs = nowUs;
    return [held];
  }

  /// Forgets the held move without sending it. For a session that is going
  /// away, where sending it would be pointless.
  void reset() {
    _pending = null;
    _lastSentUs = null;
  }
}
