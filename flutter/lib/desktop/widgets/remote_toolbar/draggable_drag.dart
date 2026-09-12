part of 'remote_toolbar.dart';

extension _DraggableShowHideDrag on _DraggableShowHideState {
  // Bias applied to the currently-previewed edge so a drag hovering between
  // two edges doesn't flicker. Only relevant when multi-edge is enabled.
  static const double _switchHysteresisPx = 50.0;

  _ToolbarEdge _nearestToolbarEdge(Offset cursor, Size mediaSize) {
    if (!widget.multiEdgeEnabled) return widget.edge.value;

    double rawDist(_ToolbarEdge e) {
      switch (e) {
        case _ToolbarEdge.top:
          return cursor.dy;
        case _ToolbarEdge.bottom:
          return mediaSize.height - cursor.dy;
        case _ToolbarEdge.left:
          return cursor.dx;
        case _ToolbarEdge.right:
          return mediaSize.width - cursor.dx;
      }
    }

    final previewed = widget.previewEdge.value;
    var winner = widget.edge.value;
    var best = double.infinity;
    for (final e in _ToolbarEdge.values) {
      final biased =
          e == previewed ? rawDist(e) - _switchHysteresisPx : rawDist(e);
      if (biased < best) {
        best = biased;
        winner = e;
      }
    }
    return winner;
  }

  void _ensureDragGrabOffset(Offset cursor) {
    if (_dragGrabOffset != null) return;
    final mediaSize = MediaQueryData.fromView(View.of(context)).size;
    final toolbarSize =
        _toolbarSizeForEdge(widget.edge.value, widget.toolbarSize.value);
    _dragToolbarSize = toolbarSize;
    final toolbarOffset = _toolbarOffsetForEdge(
      edge: widget.edge.value,
      fraction: widget.fraction.value,
      parentSize: mediaSize,
      toolbarSize: toolbarSize,
    );
    _dragGrabOffset = cursor - toolbarOffset;
    _dragLongAxisGrabOffset = _isHorizontalEdge(widget.edge.value)
        ? _dragGrabOffset?.dx
        : _dragGrabOffset?.dy;
  }

  double _dragGrabOffsetForEdge(_ToolbarEdge edge, Size toolbarSize) {
    final offset = _dragLongAxisGrabOffset ?? 0;
    final extent =
        _isHorizontalEdge(edge) ? toolbarSize.width : toolbarSize.height;
    return _clampToolbarFraction(offset, 0, extent);
  }

  void _updatePreview(Offset cursor) {
    _ensureDragGrabOffset(cursor);
    final mediaSize = MediaQueryData.fromView(View.of(context)).size;
    final winner = _nearestToolbarEdge(cursor, mediaSize);
    widget.previewEdge.value = winner;

    final toolbarSize = _toolbarSizeForEdge(winner, _dragToolbarSize);
    final grabOffset = _dragGrabOffsetForEdge(winner, toolbarSize);
    final double frac;
    if (winner == _ToolbarEdge.top || winner == _ToolbarEdge.bottom) {
      frac = _fractionForAlignedDrag(
        cursor: cursor.dx,
        grabOffset: grabOffset,
        parentExtent: mediaSize.width,
        toolbarExtent: toolbarSize.width,
        left: left,
        right: right,
      );
    } else {
      final fractionBounds = _fractionBoundsForEdge(winner, left, right);
      frac = _fractionForAlignedDrag(
        cursor: cursor.dy,
        grabOffset: grabOffset,
        parentExtent: mediaSize.height,
        toolbarExtent: toolbarSize.height,
        left: fractionBounds.left,
        right: fractionBounds.right,
      );
    }
    widget.previewFraction.value = frac;
  }

  void _resetDragTracking() {
    _lastPointerDown = null;
    _dragGrabOffset = null;
    _dragLongAxisGrabOffset = null;
    _dragToolbarSize = null;
  }

  void _commitPreview() {
    final newEdge = widget.previewEdge.value;
    final frac = widget.previewFraction.value;
    widget.previewEdge.value = null;
    widget.previewFraction.value = null;
    widget.dragging.value = false;
    widget.markDragEpoch();
    _resetDragTracking();
    widget.syncDockingOptionsAfterDragIfNeeded();
    if (newEdge == null || frac == null) return;
    widget.edge.value = newEdge;
    widget.fraction.value = frac;
    _cacheToolbarDockingOptions(
      sessionId: widget.sessionId,
      edge: newEdge,
      fraction: frac,
      multiEdgeEnabled: widget.multiEdgeEnabled,
    );
    bind.sessionPeerOption(
      sessionId: widget.sessionId,
      name: kOptionRemoteMenubarEdge,
      value: _toolbarEdgeToString(newEdge),
    );
    bind.sessionPeerOption(
      sessionId: widget.sessionId,
      name: kOptionRemoteMenubarFraction,
      value: frac.toString(),
    );
    if (widget.multiEdgeEnabled) {
      return;
    }
    bind.sessionPeerOption(
      sessionId: widget.sessionId,
      name: _legacyRemoteMenubarDragX,
      value: frac.toString(),
    );
  }
}
