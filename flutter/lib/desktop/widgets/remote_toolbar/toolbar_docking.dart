part of 'remote_toolbar.dart';

extension _RemoteToolbarDocking on _RemoteToolbarState {
  Future<void> _syncDockingOptions({required bool force}) async {
    final syncSerial = ++_dockingOptionSyncSerial;
    if (_dragging.isTrue) {
      _deferDockingOptionsSync();
      return;
    }
    final dragEpoch = _dragEpoch;

    // Use the canonical helper so the option's documented default semantics
    // apply (allow-* prefix => default false). Keeping it raw-string would
    // diverge from how _OptionCheckBox displays the same key.
    final multiEdgeEnabled =
        mainGetLocalBoolOptionSync(kOptionAllowMultiEdgeToolbarDock);
    final cached = _cachedToolbarDockingOptions(widget.ffi.sessionId);
    if (cached == null && pi.isSet.isFalse) {
      return;
    }
    final hadDockingOptions = cached != null;
    final wasMultiEdgeEnabled =
        cached?.multiEdgeEnabled ?? _multiEdgeEnabled.value;
    if (!force &&
        hadDockingOptions &&
        wasMultiEdgeEnabled == multiEdgeEnabled) {
      _pendingDockingOptionSync = false;
      return;
    }

    final savedFraction = await bind.sessionGetOption(
        sessionId: widget.ffi.sessionId, arg: kOptionRemoteMenubarFraction);
    // Backward compat: legacy horizontal-only position.
    final legacyFraction = await bind.sessionGetOption(
        sessionId: widget.ffi.sessionId, arg: _legacyRemoteMenubarDragX);
    if (!mounted || syncSerial != _dockingOptionSyncSerial) return;

    var nextEdge = _edge.value;
    var savedFractionForNextEdge = savedFraction;
    var keepCurrentPosition = false;
    if (!multiEdgeEnabled) {
      nextEdge = _ToolbarEdge.top;
    } else if (force || wasMultiEdgeEnabled || cached == null) {
      final edgeStr = await bind.sessionGetOption(
          sessionId: widget.ffi.sessionId, arg: kOptionRemoteMenubarEdge);
      if (!mounted || syncSerial != _dockingOptionSyncSerial) return;
      nextEdge = _parseToolbarEdge(edgeStr);
    } else {
      // The setting changed from top-only to multi-edge while this toolbar is
      // already visible. Keep its current position instead of jumping to the
      // last saved multi-edge dock.
      nextEdge = cached.edge;
      savedFractionForNextEdge = cached.fraction.toString();
      keepCurrentPosition = true;
    }

    final rawFraction = _toolbarRawFraction(
      multiEdgeEnabled: multiEdgeEnabled,
      edge: nextEdge,
      savedFraction: savedFractionForNextEdge,
      legacyFraction: legacyFraction,
    );
    // Clamp to the saved drag-bound contract so a corrupted or out-of-range
    // saved value can't bypass it until the user drags again.
    final dragLeft = double.tryParse(
            bind.mainGetLocalOption(key: kOptionRemoteMenubarDragLeft)) ??
        0.0;
    final dragRight = double.tryParse(
            bind.mainGetLocalOption(key: kOptionRemoteMenubarDragRight)) ??
        1.0;
    final fractionBounds =
        _fractionBoundsForEdge(nextEdge, dragLeft, dragRight);
    final nextFraction = (double.tryParse(rawFraction) ?? 0.5)
        .clamp(fractionBounds.left, fractionBounds.right)
        .toDouble();
    if (!mounted || syncSerial != _dockingOptionSyncSerial) return;
    if (_dragging.isTrue || dragEpoch != _dragEpoch) {
      _deferDockingOptionsSync();
      return;
    }
    _edge.value = nextEdge;
    _fraction.value = nextFraction;
    _multiEdgeEnabled.value = multiEdgeEnabled;
    _dockingOptionsInitialized.value = true;
    _cacheToolbarDockingOptions(
      sessionId: widget.ffi.sessionId,
      edge: nextEdge,
      fraction: nextFraction,
      multiEdgeEnabled: multiEdgeEnabled,
    );
    _pendingDockingOptionSync = false;
    if (!multiEdgeEnabled || keepCurrentPosition) {
      bind.sessionPeerOption(
        sessionId: widget.ffi.sessionId,
        name: kOptionRemoteMenubarEdge,
        value: _toolbarEdgeToString(nextEdge),
      );
      bind.sessionPeerOption(
        sessionId: widget.ffi.sessionId,
        name: kOptionRemoteMenubarFraction,
        value: nextFraction.toString(),
      );
    }
  }

  void _deferDockingOptionsSync() {
    _pendingDockingOptionSync = true;
    if (_dragging.isFalse) {
      _syncDockingOptionsAfterDragIfNeeded();
    }
  }

  void _markToolbarDragEpoch() {
    ++_dragEpoch;
  }

  void _syncDockingOptionsAfterDragIfNeeded() {
    if (!_pendingDockingOptionSync) return;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await _syncDockingOptions(force: false);
    });
  }
}
