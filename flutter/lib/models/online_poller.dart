import 'dart:async';

import 'online_presence.dart';
import 'platform_model.dart';

/// Asks the server which of the listed devices are reachable, and remembers
/// what it answered.
///
/// The device list used to read a flag nobody filled: the batch query lives
/// in the legacy peers view, which the desktop home does not mount, so every
/// device stayed at the flag's default and the page called that "offline or
/// unknown". This issues the query the page needs and keeps the third
/// answer, "not asked yet", apart from the other two.
class OnlinePoller {
  OnlinePoller({
    required this.name,
    this.onChanged,
    Future<void> Function(List<String> ids)? query,
    Future<bool> Function()? usingPublicServer,
  })  : _query = query ?? ((ids) => bind.queryOnlines(ids: ids)),
        _usingPublicServer =
            usingPublicServer ?? (() => bind.mainIsUsingPublicServer());

  /// Distinguishes this listener from the other handlers of the same event.
  final String name;
  final void Function()? onChanged;

  static const String _event = 'callback_query_onlines';

  /// The intervals the legacy peers view uses: a public server is asked
  /// rarely, a deployment's own server often.
  static const Duration publicInterval = Duration(seconds: 20);
  static const Duration ownInterval = Duration(seconds: 6);

  final Future<void> Function(List<String> ids) _query;
  final Future<bool> Function() _usingPublicServer;
  final PresenceBook _book = PresenceBook();
  OnlinePollSchedule _schedule = OnlinePollSchedule(interval: publicInterval);
  Timer? _timer;
  Set<String> _ids = const {};
  bool _visible = true;
  bool _started = false;

  PeerPresence presenceOf(String id) => _book.of(id);

  /// The ids currently on screen. Cheap to call on every rebuild.
  void watch(Iterable<String> ids) {
    _ids = ids.toSet();
    _tick();
  }

  /// A page that is no longer shown asks nothing until it is shown again.
  set visible(bool value) {
    if (_visible == value) return;
    _visible = value;
    _tick();
  }

  void start() {
    if (_started) return;
    _started = true;
    platformFFI.registerEventHandler(_event, name, (evt) async {
      _book.record(
        onlines: parsePeerIdList(evt['onlines']),
        offlines: parsePeerIdList(evt['offlines']),
      );
      onChanged?.call();
    });
    () async {
      final public = await _usingPublicServer();
      _schedule = OnlinePollSchedule(
          interval: public ? publicInterval : ownInterval);
    }();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _tick());
    _tick();
  }

  void dispose() {
    _timer?.cancel();
    _timer = null;
    if (_started) {
      platformFFI.unregisterEventHandler(_event, name);
      _started = false;
    }
  }

  void _tick() {
    if (!_started) return;
    final now = DateTime.now();
    if (!_schedule.due(ids: _ids, visible: _visible, now: now)) return;
    _schedule.sent(_ids, now);
    _query(_ids.toList(growable: false));
  }
}
