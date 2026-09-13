part of 'remote_page.dart';

/// Drives the [SessionStatusBar] from the session's own observables: peer
/// info plus the first image end "connecting", and the connection type tells
/// a direct session from a relayed one.
extension _RemotePageStatus on _RemotePageState {
  void _initStatusBar() {
    _statusWorkers.addAll([
      ever(_ffi.ffiModel.pi.isSet, (_) => _refreshStatus()),
      ever(_ffi.ffiModel.waitForFirstImage, (_) => _refreshStatus()),
      ever(ConnectionTypeState.find(widget.id).direct, (_) => _refreshStatus()),
    ]);
    _refreshStatus();
  }

  void _refreshStatus() {
    _statusController.readOnly.value = widget.viewOnly || _ffi.viewOnlySession;
    final connected = _ffi.ffiModel.pi.isSet.isTrue &&
        _ffi.ffiModel.waitForFirstImage.isFalse;
    if (!connected) {
      if (_statusController.phase.value != SessionPhase.connecting) {
        _statusController.connecting();
      }
      return;
    }
    final direct = ConnectionTypeState.find(widget.id).direct.value ==
        ConnectionType.strDirect;
    if (_statusController.phase.value == SessionPhase.connected) {
      _statusController.direct.value = direct;
    } else {
      _statusController.connected(direct: direct);
    }
  }

  void _disposeStatusBar() {
    for (final worker in _statusWorkers) {
      worker.dispose();
    }
    _statusWorkers.clear();
    _statusController.dispose();
  }
}
