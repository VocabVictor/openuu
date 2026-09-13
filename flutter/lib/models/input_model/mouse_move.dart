part of 'input_model.dart';

extension InputModelMouseMove on InputModel {
  void enterOrLeave(bool enter) {
    flushPendingMove();
    toReleaseKeys.release(handleKeyEvent);
    toReleaseRawKeys.release(handleRawKeyEvent);
    _pointerMovedAfterEnter = false;
    _pointerInsideImage = enter;
    _lastWheelTsUs = 0;

    // Track active model for side button events (Linux).
    SideButtons.onEnterOrLeave(this, enter);

    // Fix status
    if (!enter) {
      resetModifiers();
    }
    _relativeMouse.onEnterOrLeaveImage(enter);
    _flingTimer?.cancel();
    if (!isInputSourceFlutter) {
      bind.sessionEnterOrLeave(sessionId: sessionId, enter: enter);
    }
    if (enter) {
      bind.setCurSessionId(sessionId: sessionId);
    }
  }

  /// Send mouse movement event with distance in [x] and [y].
  Future<void> moveMouse(double x, double y) async {
    if (!keyboardPerm) return;
    if (isViewCamera) return;
    var x2 = x.toInt();
    var y2 = y.toInt();
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify({'x': '$x2', 'y': '$y2'})));
  }

  /// Send relative mouse movement for mobile clients (virtual joystick).
  /// This method is for touch-based controls that want to send delta values.
  /// Uses the 'move_relative' type which bypasses absolute position tracking.
  ///
  /// Accumulates fractional deltas to avoid losing slow/fine movements.
  /// Only sends events when relative mouse mode is enabled and supported.
  Future<void> sendMobileRelativeMouseMove(double dx, double dy) async {
    if (!keyboardPerm) return;
    if (isViewCamera) return;
    // Only send relative mouse events when relative mode is enabled and supported.
    if (!isRelativeMouseModeSupported || !relativeMouseMode.value) return;
    _mobileDeltaRemainderX += dx;
    _mobileDeltaRemainderY += dy;
    final x = _mobileDeltaRemainderX.truncate();
    final y = _mobileDeltaRemainderY.truncate();
    _mobileDeltaRemainderX -= x;
    _mobileDeltaRemainderY -= y;
    if (x == 0 && y == 0) return;
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify({
          'type': 'move_relative',
          'x': '$x',
          'y': '$y',
        })));
  }

  /// Update the pointer lock center position based on current window frame.
  Future<void> updatePointerLockCenter({Offset? localCenter}) {
    return _relativeMouse.updatePointerLockCenter(localCenter: localCenter);
  }

  /// Get the current image widget size (for comparison to avoid unnecessary updates).
  Size? get imageWidgetSize => _relativeMouse.imageWidgetSize;

  /// Update the image widget size for center calculation.
  void updateImageWidgetSize(Size size) {
    _relativeMouse.updateImageWidgetSize(size);
  }

  void toggleRelativeMouseMode() {
    _relativeMouse.toggleRelativeMouseMode();
  }

  bool setRelativeMouseMode(bool enabled) {
    return _relativeMouse.setRelativeMouseMode(enabled);
  }

  /// Exit relative mouse mode and release all modifier keys to the remote.
  /// This is called when the user presses the exit shortcut (Ctrl+Alt on Win/Linux, Cmd+G on macOS).
  /// We need to send key-up events for all modifiers because the shortcut itself may have
  /// blocked some key events, leaving the remote in a state where modifiers are stuck.
  void exitRelativeMouseModeWithKeyRelease() {
    if (!_relativeMouse.enabled.value) return;

    // First, send release events for all modifier keys to the remote.
    // This ensures the remote doesn't have stuck modifier keys after exiting.
    // Use press: false, down: false to send key-up events without modifiers attached.
    final modifiersToRelease = [
      'Control_L',
      'Control_R',
      'Alt_L',
      'Alt_R',
      'Shift_L',
      'Shift_R',
      'Meta_L', // Command/Super left
      'Meta_R', // Command/Super right
    ];

    for (final key in modifiersToRelease) {
      bind.sessionInputKey(
        sessionId: sessionId,
        name: key,
        down: false,
        press: false,
        alt: false,
        ctrl: false,
        shift: false,
        command: false,
      );
    }

    // Reset local modifier state
    resetModifiers();

    // Now exit relative mouse mode
    _relativeMouse.setRelativeMouseMode(false);
  }

  void disposeRelativeMouseMode() {
    _relativeMouse.dispose();
    onRelativeMouseModeDisabled = null;
    // Cancel the relative mouse mode observer and clean up global state.
    _relativeMouseModeDisposer?.dispose();
    _relativeMouseModeDisposer = null;
    final peerId = id;
    if (peerId.isNotEmpty) {
      stateGlobal.relativeMouseModeState.remove(peerId);
    }
  }

  void onWindowBlur() {
    _relativeMouse.onWindowBlur();
  }

  void onWindowFocus() {
    _relativeMouse.onWindowFocus();
  }
}
