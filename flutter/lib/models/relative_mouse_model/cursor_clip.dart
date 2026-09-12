part of 'relative_mouse_model.dart';

extension RelativeMouseCursorClip on RelativeMouseModel {
  void _applyCursorClipForFrame(Rect frame) {
    if (!isWindows) return;

    // Use PlatformDispatcher to get the device pixel ratio for proper scaling.
    final view = ui.PlatformDispatcher.instance.views.firstOrNull;
    final scale = view?.devicePixelRatio ?? 1.0;

    // Get the Flutter view's physical size (client area in pixels).
    final clientPhysical = view?.physicalSize ?? ui.Size.zero;

    // Calculate the non-client area (OS window title bar, borders).
    // frame includes the entire window (title bar + borders + client area).
    final extraW = frame.width - clientPhysical.width;
    final extraH = frame.height - clientPhysical.height;

    // Assume symmetric horizontal borders.
    final borderX = extraW > 0 ? extraW / 2 : 0.0;
    // Bottom border is typically the same as side borders.
    final borderBottom = borderX;
    // OS window title bar height is the remaining vertical non-client space.
    final borderTop = extraH > borderBottom ? extraH - borderBottom : 0.0;

    // Calculate client area top-left in screen coordinates.
    final clientTopLeftScreen =
        Offset(frame.left + borderX, frame.top + borderTop);

    int left, top, right, bottom;

    // If we have precise image widget info, clip to the remote image area.
    // This excludes the Flutter app's internal title bar and toolbar.
    if (_pointerRegionTopLeftGlobal != null &&
        _imageWidgetSize != null &&
        scale > 0) {
      // _pointerRegionTopLeftGlobal is in Flutter logical coordinates (relative to client area).
      // Convert to screen physical coordinates.
      left = (clientTopLeftScreen.dx + _pointerRegionTopLeftGlobal!.dx * scale)
          .toInt();
      top = (clientTopLeftScreen.dy + _pointerRegionTopLeftGlobal!.dy * scale)
          .toInt();
      right = (left + _imageWidgetSize!.width * scale).toInt();
      bottom = (top + _imageWidgetSize!.height * scale).toInt();
    } else {
      // Fallback: clip to client area (excluding OS window decorations).
      left = clientTopLeftScreen.dx.toInt();
      top = clientTopLeftScreen.dy.toInt();
      right = (frame.left + frame.width - borderX).toInt();
      bottom = (frame.top + frame.height - borderBottom).toInt();
    }

    _cursorClipApplied = bind.mainClipCursor(
      left: left,
      top: top,
      right: right,
      bottom: bottom,
      enable: true,
    );
  }

  void _releaseCursorClip() {
    if (!_cursorClipApplied) return;
    _cursorClipApplied = false;
    if (!isWindows) return;

    bind.mainClipCursor(
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
      enable: false,
    );
  }

  void _resetState() {
    // Flush any pending delta before clearing state.
    // This ensures the last buffered movement is sent before values are zeroed.
    // Fire-and-forget: we don't wait for the async send to complete.
    if (_throttleTimer != null || _pendingDeltaX != 0 || _pendingDeltaY != 0) {
      _throttleTimer?.cancel();
      _throttleTimer = null;
      if (_pendingDeltaX != 0 || _pendingDeltaY != 0) {
        final x = _pendingDeltaX;
        final y = _pendingDeltaY;
        _pendingDeltaX = 0;
        _pendingDeltaY = 0;
        // Send without awaiting; skip recenter since we're disabling.
        _sendMouseMessageToSession({
          'type': 'move_relative',
          'x': '$x',
          'y': '$y',
        }, disableRelativeOnError: false);
      }
    }
    _accumulator.reset();
    _pointerLockCenterLocal = null;
    _pointerLockCenterScreen = null;
    _pointerRegionTopLeftGlobal = null;
    _lastPointerLocalPos = null;
    _skipNextMouseMove = false;
    setPointerInsideImage(false);
    _cursorClipApplied = false;
    _exitShortcutKeyDown = false;
  }

  /// Core cleanup logic shared by [_disableWithCleanup] and [dispose].
  /// Sends disable message to Rust, releases platform resources, and resets state.
  void _performCleanupCore() {
    // Best-effort marker for Rust rdev grab loop (ESC behavior).
    // Bypass keyboardPerm check to ensure Rust state is always synced.
    _sendMouseMessageToSession(
      {
        'relative_mouse_mode': '0',
      },
      disableRelativeOnError: false,
      bypassKeyboardPerm: true,
    );

    // macOS: Disable native relative mouse mode
    // This already calls CGAssociateMouseAndMouseCursorPosition(1) to re-associate mouse
    if (isMacOS) {
      _disableNativeRelativeMouseMode();
    } else {
      _releaseCursorClip();
    }

    _resetState();
  }
}
