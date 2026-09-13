/// What is known about whether a peer is reachable.
///
/// The previous device page had a bool, which cannot hold the state the
/// device actually spends its first seconds in: nobody has asked the server
/// yet. A bool forces the two negatives together, and the page said
/// "offline or unknown" because that was the only honest thing a bool could
/// say. Three values let each of them be said on its own.
library;

enum PeerPresence { online, offline, unknown }

/// When the next batch query is due. Pure: the caller supplies the clock.
class OnlinePollSchedule {
  OnlinePollSchedule({required this.interval});

  final Duration interval;

  DateTime? _lastSent;
  Set<String> _lastIds = const {};

  DateTime? get lastSent => _lastSent;

  /// A query is worth sending when there is something to ask about, someone
  /// to see the answer, and either the question changed or the interval has
  /// passed. An empty list or a page nobody is looking at asks nothing: the
  /// poll must not spin in the background.
  bool due({
    required Set<String> ids,
    required bool visible,
    required DateTime now,
  }) {
    if (!visible || ids.isEmpty) {
      return false;
    }
    if (!_sameIds(ids)) {
      return true;
    }
    final last = _lastSent;
    return last == null || now.difference(last) >= interval;
  }

  void sent(Set<String> ids, DateTime now) {
    _lastIds = {...ids};
    _lastSent = now;
  }

  bool _sameIds(Set<String> ids) =>
      ids.length == _lastIds.length && ids.every(_lastIds.contains);
}

/// Which peers the server has answered for, so a peer that has not been
/// asked about yet is not reported as offline.
class PresenceBook {
  final Set<String> _answered = {};
  final Set<String> _online = {};

  /// One reply from the server: the ids it says are up, and the ids it says
  /// are down. Both count as an answer about that id.
  void record({
    Iterable<String> onlines = const [],
    Iterable<String> offlines = const [],
  }) {
    for (final id in onlines) {
      if (id.isEmpty) continue;
      _answered.add(id);
      _online.add(id);
    }
    for (final id in offlines) {
      if (id.isEmpty) continue;
      _answered.add(id);
      _online.remove(id);
    }
  }

  PeerPresence of(String id) {
    if (!_answered.contains(id)) {
      return PeerPresence.unknown;
    }
    return _online.contains(id) ? PeerPresence.online : PeerPresence.offline;
  }

  bool get isEmpty => _answered.isEmpty;

  /// Whatever was known is stale once the connection to the server is: a
  /// stale "online" is worse than saying nothing.
  void clear() {
    _answered.clear();
    _online.clear();
  }
}

/// The server's reply carries the two lists as comma-separated strings.
Iterable<String> parsePeerIdList(Object? value) {
  if (value is! String || value.isEmpty) {
    return const [];
  }
  return value.split(',').map((e) => e.trim()).where((e) => e.isNotEmpty);
}
