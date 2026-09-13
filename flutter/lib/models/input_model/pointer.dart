part of 'input_model.dart';

extension InputModelPointer on InputModel {
  Offset setNearestEdge(double x, double y, Rect rect) {
    double left = x - rect.left;
    double right = rect.right - 1 - x;
    double top = y - rect.top;
    double bottom = rect.bottom - 1 - y;
    if (left < right && left < top && left < bottom) {
      x = rect.left;
    }
    if (right < left && right < top && right < bottom) {
      x = rect.right - 1;
    }
    if (top < left && top < right && top < bottom) {
      y = rect.top;
    }
    if (bottom < left && bottom < right && bottom < top) {
      y = rect.bottom - 1;
    }
    return Offset(x, y);
  }

  void handlePointerEvent(String kind, String type, Offset offset) {
    double x = offset.dx;
    double y = offset.dy;
    if (_checkPeerControlProtected(x, y)) {
      return;
    }
    // Only touch events are handled for now. So we can just ignore buttons.
    // to-do: handle mouse events

    late final dynamic evtValue;
    if (type == kMouseEventTypePanUpdate) {
      evtValue = {
        'x': x.toInt(),
        'y': y.toInt(),
      };
    } else {
      final isMoveTypes = [kMouseEventTypePanStart, kMouseEventTypePanEnd];
      final pos = handlePointerDevicePos(
        kPointerEventKindTouch,
        x,
        y,
        isMoveTypes.contains(type),
        type,
      );
      if (pos == null) {
        return;
      }
      evtValue = {
        'x': pos.x.toInt(),
        'y': pos.y.toInt(),
      };
    }

    final evt = PointerEventToRust(kind, type, evtValue).toJson();
    if (isViewCamera) return;
    bind.sessionSendPointer(
        sessionId: sessionId, msg: json.encode(modify(evt)));
  }

  bool _checkPeerControlProtected(double x, double y) {
    if (isViewOnly && showMyCursor) {
      lastMousePos = ui.Offset(x, y);
      return false;
    }

    final cursorModel = parent.target!.cursorModel;
    if (cursorModel.isPeerControlProtected) {
      lastMousePos = ui.Offset(x, y);
      return true;
    }

    if (!cursorModel.gotMouseControl) {
      bool selfGetControl =
          (x - lastMousePos.dx).abs() > kMouseControlDistance ||
              (y - lastMousePos.dy).abs() > kMouseControlDistance;
      if (selfGetControl) {
        cursorModel.gotMouseControl = true;
      } else {
        lastMousePos = ui.Offset(x, y);
        return true;
      }
    }
    lastMousePos = ui.Offset(x, y);
    return false;
  }

  Map<String, dynamic>? processEventToPeer(
    Map<String, dynamic> evt,
    Offset offset, {
    bool onExit = false,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    if (isViewCamera) return null;
    double x = offset.dx;
    double y = max(0.0, offset.dy);
    if (_checkPeerControlProtected(x, y)) {
      return null;
    }

    var type = kMouseEventTypeDefault;
    var isMove = false;
    switch (evt['type']) {
      case _kMouseEventDown:
        type = kMouseEventTypeDown;
        break;
      case _kMouseEventUp:
        type = kMouseEventTypeUp;
        break;
      case _kMouseEventMove:
        _pointerMovedAfterEnter = true;
        isMove = true;
        break;
      default:
        return null;
    }
    evt['type'] = type;

    if (type == kMouseEventTypeDown && !_pointerMovedAfterEnter) {
      // Move mouse to the position of the down event first.
      lastMousePos = ui.Offset(x, y);
      refreshMousePos();
    }

    final pos = handlePointerDevicePos(
      kPointerEventKindMouse,
      x,
      y,
      isMove,
      type,
      onExit: onExit,
      buttons: evt['buttons'],
      moveCanvas: moveCanvas,
      edgeScroll: edgeScroll,
    );
    if (pos == null) {
      return null;
    }
    if (type != '') {
      evt['x'] = '0';
      evt['y'] = '0';
    } else {
      evt['x'] = '${pos.x.toInt()}';
      evt['y'] = '${pos.y.toInt()}';
    }

    final buttons = evt['buttons'];
    if (buttons is int) {
      evt['buttons'] = mouseButtonsToPeer(buttons);
    } else {
      // Log warning if buttons exists but is not an int (unexpected caller).
      // Keep empty string fallback for missing buttons to preserve move/hover behavior.
      if (buttons != null) {
        debugPrint(
            '[InputModel] processEventToPeer: unexpected buttons type: ${buttons.runtimeType}, value: $buttons');
      }
      evt['buttons'] = '';
    }
    return evt;
  }

  Map<String, dynamic>? handleMouse(
    Map<String, dynamic> evt,
    Offset offset, {
    bool onExit = false,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    final evtToPeer = processEventToPeer(evt, offset,
        onExit: onExit, moveCanvas: moveCanvas, edgeScroll: edgeScroll);
    if (evtToPeer != null) {
      _sendMouseCoalesced(evtToPeer);
    }
    return evtToPeer;
  }
}

extension InputModelMoveCoalesce on InputModel {
  /// A move carries no type of its own, which is what makes it a move: only
  /// those are held back.
  void _sendMouseCoalesced(Map<String, dynamic> evt) {
    final nowUs = DateTime.now().microsecondsSinceEpoch;
    final type = evt['type'];
    final isMove = type is! String || type.isEmpty;
    final out = isMove
        ? _moveCoalescer.offerMove(evt, nowUs)
        : _moveCoalescer.offerImmediate(evt, nowUs);
    for (final e in out) {
      _sendMouseNow(e);
    }
    _scheduleMoveFlush(nowUs);
  }

  void _sendMouseNow(Map<String, dynamic> evt) {
    bind.sessionSendMouse(
        sessionId: sessionId, msg: json.encode(modify(evt)));
    _countSent(evt['type']);
  }

  /// A pointer that stops moving leaves its last position held, so the
  /// interval boundary has to send it even when no further event arrives.
  void _scheduleMoveFlush(int nowUs) {
    final delayUs = _moveCoalescer.delayUs(nowUs);
    if (delayUs == null) {
      _moveFlushTimer?.cancel();
      _moveFlushTimer = null;
      return;
    }
    if (_moveFlushTimer?.isActive ?? false) {
      return;
    }
    _moveFlushTimer = Timer(Duration(microseconds: delayUs), () {
      _moveFlushTimer = null;
      flushPendingMove();
    });
  }

  /// Sends whatever move is held, now.
  void flushPendingMove() {
    for (final e
        in _moveCoalescer.flush(DateTime.now().microsecondsSinceEpoch)) {
      _sendMouseNow(e);
    }
  }
}

extension InputModelSendCount on InputModel {
  /// A move carries no type of its own, which is what makes it a move.
  void _countSent(Object? type) {
    final counter = _sendCounter;
    if (counter == null) {
      return;
    }
    final name = (type is String && type.isNotEmpty) ? type : 'move';
    final report =
        counter.record(name, DateTime.now().microsecondsSinceEpoch);
    if (report != null) {
      debugPrint('[InputModel] $report');
    }
  }
}
