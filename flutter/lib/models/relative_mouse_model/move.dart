part of 'relative_mouse_model.dart';

extension RelativeMouseSend on RelativeMouseModel {
  Future<bool> _sendMouseMessageToSession(
    Map<String, dynamic> msg, {
    bool disableRelativeOnError = true,
    bool bypassKeyboardPerm = false,
  }) async {
    if (!bypassKeyboardPerm && !keyboardPerm()) return false;
    if (isViewCamera()) return false;

    try {
      await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify(msg)),
      );
      return true;
    } catch (e) {
      debugPrint('[RelMouse] Error sending mouse message: $e');
      if (disableRelativeOnError && enabled.value) {
        _disableWithCleanup();
      }
      return false;
    }
  }
}

extension RelativeMouseEdge on RelativeMouseModel {
  /// Calculate dynamic edge threshold based on widget size.
  double _calculateEdgeThreshold(Size size) {
    final smallerDimension = math.min(size.width, size.height);
    if (isLinux) {
      // Use more aggressive thresholds on Linux to prevent cursor escape.
      final dynamicThreshold = smallerDimension * RelativeMouseModel._edgeThresholdFractionLinux;
      return dynamicThreshold.clamp(
          RelativeMouseModel._edgeThresholdMinLinux, RelativeMouseModel._edgeThresholdMaxLinux);
    }
    final dynamicThreshold = smallerDimension * RelativeMouseModel._edgeThresholdFraction;
    // Clamp between min and max thresholds
    return dynamicThreshold.clamp(RelativeMouseModel._edgeThresholdMin, RelativeMouseModel._edgeThresholdMax);
  }

  /// Recenter the cursor only if it's near the edge of the image widget.
  void _recenterIfNearEdge() {
    final lastPos = _lastPointerLocalPos;
    final size = _imageWidgetSize;
    if (lastPos == null || size == null) return;

    // Dynamic threshold based on widget size
    final edgeThreshold = _calculateEdgeThreshold(size);

    final nearLeft = lastPos.dx < edgeThreshold;
    final nearRight = lastPos.dx > size.width - edgeThreshold;
    final nearTop = lastPos.dy < edgeThreshold;
    final nearBottom = lastPos.dy > size.height - edgeThreshold;

    if (nearLeft || nearRight || nearTop || nearBottom) {
      _recenterMouse();
    }
  }

  /// Send mouse button event without position (for relative mouse mode).
  Future<void> sendRelativeMouseButton(Map<String, dynamic> evt) async {
    if (!enabled.value) return;
    _ensurePointerLockEngaged();

    final rawType = evt['type'];
    final rawButtons = evt['buttons'];
    if (rawType is! String || rawButtons is! int) return;

    final type = RelativeMouseModel._mouseEventTypeToPeer(rawType);
    if (type.isEmpty) return;

    final buttons = mouseButtonsToPeer(rawButtons);
    if (buttons.isEmpty) return;

    await _sendMouseMessageToSession({
      'type': type,
      'buttons': buttons,
    });
  }
}

extension RelativeMouseMove on RelativeMouseModel {
  /// Handle relative mouse movement based on current local pointer position.
  /// Returns true if the event was handled in relative mode, false otherwise.
  bool handleRelativeMouseMove(Offset localPosition) {
    if (!enabled.value) return false;

    // macOS: Native mode handles delta via callback, skip Flutter-based handling.
    if (_isNativeRelativeMouseModeActive) {
      return true;
    }

    // Pointer move/hover implies we're inside the remote image.
    _ensurePointerLockEngaged();

    // Skip the mouse move event triggered by recenter operation itself.
    if (_skipNextMouseMove) {
      _skipNextMouseMove = false;
      _lastPointerLocalPos = localPosition;
      return true;
    }

    final lastLocal = _lastPointerLocalPos;
    _lastPointerLocalPos = localPosition;

    // Linux-specific: Proactive recenter check before processing delta.
    // On Linux, we don't have clip_cursor, so if the cursor moves too fast
    // it may escape the window before _recenterIfNearEdge can catch it.
    // Check now and recenter immediately if needed.
    if (isLinux) {
      _recenterIfNearEdgeLinux(localPosition);
    }

    // Calculate delta from last position (not from center).
    // This avoids issues with CGWarpMouseCursorPosition integer rounding.
    if (lastLocal != null) {
      final delta = localPosition - lastLocal;
      if (delta.dx != 0 || delta.dy != 0) {
        sendRelativeMouseMove(delta.dx, delta.dy);
      }
    }

    return true;
  }

  /// Linux-specific: More aggressive recenter check to prevent cursor escape.
  /// Called synchronously before processing mouse delta to ensure cursor stays within bounds.
  void _recenterIfNearEdgeLinux(Offset localPosition) {
    final size = _imageWidgetSize;
    if (size == null) return;

    final edgeThreshold = _calculateEdgeThreshold(size);

    final nearLeft = localPosition.dx < edgeThreshold;
    final nearRight = localPosition.dx > size.width - edgeThreshold;
    final nearTop = localPosition.dy < edgeThreshold;
    final nearBottom = localPosition.dy > size.height - edgeThreshold;

    if (nearLeft || nearRight || nearTop || nearBottom) {
      _recenterMouse();
    }
  }

  void sendRelativeMouseMove(double dx, double dy) {
    if (!isDesktop) return;

    final delta = _accumulator.add(dx, dy, maxDelta: kMaxRelativeMouseDelta);
    if (delta == null) return;

    // Buffer the delta for throttled sending.
    _pendingDeltaX += delta.x;
    _pendingDeltaY += delta.y;

    // Start or refresh the throttle timer.
    if (_throttleTimer == null || !_throttleTimer!.isActive) {
      _throttleTimer = Timer(RelativeMouseModel._throttleInterval, () => _flushPendingDelta());
    }
  }

  Future<void> _flushPendingDelta() async {
    if (!isDesktop) return;
    if (_pendingDeltaX == 0 && _pendingDeltaY == 0) return;

    final x = _pendingDeltaX;
    final y = _pendingDeltaY;
    _pendingDeltaX = 0;
    _pendingDeltaY = 0;

    final ok = await _sendMouseMessageToSession({
      'type': 'move_relative',
      'x': '$x',
      'y': '$y',
    });
    if (!ok) return;

    // Only recenter when mouse is near the edge of the image widget.
    // This allows smooth mouse movement without constant recentering.
    _recenterIfNearEdge();
  }
}
