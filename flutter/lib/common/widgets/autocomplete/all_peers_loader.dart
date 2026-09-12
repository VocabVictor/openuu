part of 'autocomplete.dart';

class AllPeersLoader {
  List<Peer> peers = [];

  bool _isPeersLoading = false;
  bool _isPeersLoaded = false;
  Set<String> _lastQueryOnlineIds = {};
  DateTime _lastQueryOnlineTime = DateTime.fromMillisecondsSinceEpoch(0);
  Timer? _queryOnlineTimer;
  List<Peer> _lastQueryOnlineOptions = const [];
  Set<String> _lastOnlineIds = {};
  Set<String> _lastOfflineIds = {};
  final Future<void> Function(List<String> ids) _queryOnlines;
  final Duration _queryOnlineDebounce;
  void Function(VoidCallback)? _setState;
  bool _isCleared = false;

  final String _listenerKey = 'AllPeersLoader';
  static const String _cbQueryOnlines = 'callback_query_onlines';
  static const Duration _queryOnlineInterval = Duration(seconds: 5);
  static const Duration _defaultQueryOnlineDebounce =
      Duration(milliseconds: 300);
  static const int _maxQueryOnlineOptions = 20;

  bool get needLoad => !_isPeersLoaded && !_isPeersLoading;
  bool get isPeersLoaded => _isPeersLoaded;

  AllPeersLoader({
    @visibleForTesting Future<void> Function(List<String> ids)? queryOnlines,
    @visibleForTesting Duration? queryOnlineDebounce,
  })  : _queryOnlines = queryOnlines ?? ((ids) => bind.queryOnlines(ids: ids)),
        _queryOnlineDebounce =
            queryOnlineDebounce ?? _defaultQueryOnlineDebounce;

  void init(void Function(VoidCallback) setState) {
    _setState = setState;
    _isCleared = false;
    gFFI.recentPeersModel.addListener(_mergeAllPeers);
    gFFI.lanPeersModel.addListener(_mergeAllPeers);
    gFFI.abModel.addPeerUpdateListener(_listenerKey, _mergeAllPeers);
    gFFI.groupModel.addPeerUpdateListener(_listenerKey, _mergeAllPeers);
    platformFFI.registerEventHandler(_cbQueryOnlines, _listenerKey,
        (evt) async {
      _updateOnlineState(evt);
    });
  }

  void clear() {
    gFFI.recentPeersModel.removeListener(_mergeAllPeers);
    gFFI.lanPeersModel.removeListener(_mergeAllPeers);
    gFFI.abModel.removePeerUpdateListener(_listenerKey);
    gFFI.groupModel.removePeerUpdateListener(_listenerKey);
    platformFFI.unregisterEventHandler(_cbQueryOnlines, _listenerKey);
    _queryOnlineTimer?.cancel();
    _lastQueryOnlineOptions = const [];
    _setState = null;
    _isCleared = true;
  }

  Future<void> getAllPeers() async {
    if (!needLoad) {
      return;
    }
    _isPeersLoading = true;

    if (gFFI.recentPeersModel.peers.isEmpty) {
      bind.mainLoadRecentPeers();
    }
    if (gFFI.lanPeersModel.peers.isEmpty) {
      bind.mainLoadLanPeers();
    }
    // No need to care about peers from abModel, and group model.
    // Because they will pull data in `refreshCurrentUser()` on startup.

    final startTime = DateTime.now();
    _mergeAllPeers();
    final diffTime = DateTime.now().difference(startTime).inMilliseconds;
    if (diffTime < 100) {
      await Future.delayed(Duration(milliseconds: diffTime));
    }
  }

  void _mergeAllPeers() {
    if (_isCleared) {
      return;
    }
    peers = mergeAutocompletePeers(
      addressBookPeers: gFFI.abModel.allPeers(),
      groupPeers: gFFI.groupModel.peers,
      lanPeers: gFFI.lanPeersModel.peers,
      recentPeers: gFFI.recentPeersModel.peers,
      restRecentPeerIds: gFFI.recentPeersModel.restPeerIds,
    );
    _applyLastOnlineState(peers);
    _scheduleSetState(() {
      _isPeersLoading = false;
      _isPeersLoaded = true;
    });
  }

  void _updateOnlineState(Map<String, dynamic> evt) {
    if (_isCleared) {
      return;
    }
    _lastOnlineIds = _splitPeerIds(evt['onlines']);
    _lastOfflineIds = _splitPeerIds(evt['offlines']);
    final peersChanged = _applyLastOnlineState(peers);
    final optionsChanged = _applyLastOnlineState(_lastQueryOnlineOptions);
    if (peersChanged || optionsChanged) {
      _scheduleSetState(() {});
    }
  }

  void _scheduleSetState(VoidCallback callback) {
    if (_isCleared) {
      return;
    }
    final setState = _setState;
    if (setState == null) {
      callback();
    } else {
      setState(callback);
    }
  }

  bool _applyLastOnlineState(List<Peer> peers) {
    return updateAutocompletePeerOnlineStates(
      peers,
      onlines: _lastOnlineIds,
      offlines: _lastOfflineIds,
    );
  }

  Set<String> _splitPeerIds(dynamic ids) {
    if (ids is! String || ids.isEmpty) {
      return {};
    }
    return ids.split(',').where((id) => id.isNotEmpty).toSet();
  }

  void queryOnlines(Iterable<Peer> options) {
    if (_isCleared) {
      return;
    }
    _lastQueryOnlineOptions = options.toList(growable: false);
    final ids = autocompleteOnlineQueryIds(
      _lastQueryOnlineOptions,
      limit: _maxQueryOnlineOptions,
    ).toSet();
    _queryOnlineTimer?.cancel();
    _queryOnlineTimer = null;
    if (ids.isEmpty) {
      return;
    }
    final now = DateTime.now();
    if (setEquals(ids, _lastQueryOnlineIds) &&
        now.difference(_lastQueryOnlineTime) < _queryOnlineInterval) {
      return;
    }

    _queryOnlineTimer = Timer(_queryOnlineDebounce, () async {
      try {
        await _queryOnlines(ids.toList(growable: false));
        if (_isCleared) {
          return;
        }
        _lastQueryOnlineIds = ids;
        _lastQueryOnlineTime = DateTime.now();
      } catch (e) {
        debugPrint('query autocomplete online state failed: $e');
      }
    });
  }

  @visibleForTesting
  void updateOnlineStateForTesting(Map<String, dynamic> evt) {
    _updateOnlineState(evt);
  }

  @visibleForTesting
  bool applyLastOnlineStateForTesting(List<Peer> peers) {
    return _applyLastOnlineState(peers);
  }
}
