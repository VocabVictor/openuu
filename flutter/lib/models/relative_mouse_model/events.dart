part of 'relative_mouse_model.dart';

extension RelativeMouseEvents on RelativeMouseModel {
  bool get isSupported {
    // On Linux/Wayland, cursor warping is not supported, hide the option entirely.
    if (isDesktop && isLinux && bind.mainCurrentIsWayland()) {
      return false;
    }
    // Relative mouse mode is unsupported on remote Linux:
    // 1. Long-press key events are unsupported.
    // 2. The Wayland display server lacks cursor warping support.
    final platform = peerPlatform();
    if (platform == kPeerPlatformLinux) {
      return false;
    }
    final v = peerVersion();
    if (v.isEmpty) return false;
    return versionCmp(v, kMinVersionForRelativeMouseMode) >= 0;
  }

  Size? get imageWidgetSize => _imageWidgetSize;

  void updateImageWidgetSize(Size size) {
    _imageWidgetSize = size;
    if (enabled.value) {
      _pointerLockCenterLocal = Offset(size.width / 2, size.height / 2);
    }
  }

  void updatePointerRegionTopLeftGlobal(PointerEvent e) {
    _pointerRegionTopLeftGlobal = e.position - e.localPosition;
  }

  /// Shared helper for handling exit shortcut for relative mouse mode.
  /// Returns true if the event was handled and should not be forwarded.
  ///
  /// Exit shortcuts (only work when relative mouse mode is active):
  /// - macOS: Cmd+G
  /// - Windows/Linux: Ctrl+Alt (any order - triggered when both are pressed)
  ///
  /// [logicalKey] - the logical key of the event
  /// [isKeyUp] - whether the event is a key up event
  /// [isKeyDown] - whether the event is a key down event
  /// [ctrlPressed], [altPressed], [commandPressed] - modifier states
  bool _handleExitShortcut({
    required LogicalKeyboardKey logicalKey,
    required bool isKeyUp,
    required bool isKeyDown,
    required bool ctrlPressed,
    required bool altPressed,
    required bool commandPressed,
  }) {
    if (!isDesktop || !keyboardPerm() || isViewCamera()) return false;

    // Only handle exit shortcuts when relative mouse mode is active
    if (!enabled.value) return false;

    // Block key up if key down was blocked (to avoid orphan key up event on remote).
    if (isKeyUp && _exitShortcutKeyDown) {
      _exitShortcutKeyDown = false;
      return true;
    }

    if (!isKeyDown) return false;

    // macOS: Cmd+G to exit
    if (isMacOS) {
      final isGKey = logicalKey == LogicalKeyboardKey.keyG;
      if (isGKey && commandPressed) {
        _exitShortcutKeyDown = true;
        setRelativeMouseMode(false);
        return true;
      }
      return false;
    }

    // Windows/Linux: Ctrl+Alt to exit
    // Triggered when both modifiers are pressed (check on either Ctrl or Alt key down)
    final isCtrlKey = logicalKey == LogicalKeyboardKey.controlLeft ||
        logicalKey == LogicalKeyboardKey.controlRight;
    final isAltKey = logicalKey == LogicalKeyboardKey.altLeft ||
        logicalKey == LogicalKeyboardKey.altRight;

    // When Ctrl is pressed and Alt is already down, or vice versa
    if ((isCtrlKey && altPressed) || (isAltKey && ctrlPressed)) {
      _exitShortcutKeyDown = true;
      setRelativeMouseMode(false);
      return true;
    }

    return false;
  }

  bool handleKeyEvent(
    KeyEvent e, {
    required bool ctrlPressed,
    required bool shiftPressed,
    required bool altPressed,
    required bool commandPressed,
  }) {
    return _handleExitShortcut(
      logicalKey: e.logicalKey,
      isKeyUp: e is KeyUpEvent,
      isKeyDown: e is KeyDownEvent,
      ctrlPressed: ctrlPressed,
      altPressed: altPressed,
      commandPressed: commandPressed,
    );
  }

  /// Handle raw key events for relative mouse mode.
  /// Returns true if the event was handled and should not be forwarded.
  bool handleRawKeyEvent(RawKeyEvent e) {
    final modifiers = e.data;
    return _handleExitShortcut(
      logicalKey: e.logicalKey,
      isKeyUp: e is RawKeyUpEvent,
      isKeyDown: e is RawKeyDownEvent,
      ctrlPressed: modifiers.isControlPressed,
      altPressed: modifiers.isAltPressed,
      commandPressed: modifiers.isMetaPressed,
    );
  }

  void onEnterOrLeaveImage(bool enter) {
    if (!enabled.value) return;

    // Keep the shared pointer-in-image flag in sync.
    setPointerInsideImage(enter);

    // macOS native mode: cursor is locked by CGAssociateMouseAndMouseCursorPosition,
    // no need for recenter logic.
    if (_isNativeRelativeMouseModeActive) {
      return;
    }

    if (!enter) {
      _releaseCursorClip();
      return;
    }

    // Windows: clip cursor to window rect
    // Linux: use recenter method
    updatePointerLockCenter().then((_) {
      _recenterMouse();
    });
  }

  void onWindowBlur() {
    if (!enabled.value) return;

    // Focus can change while the pointer is outside the window (e.g. taskbar activation).
    // Do not rely on the previous "pointer inside" state across focus boundaries.
    setPointerInsideImage(false);
    // macOS native mode: don't call _releaseCursorClip as it would break CGAssociateMouseAndMouseCursorPosition
    if (!_isNativeRelativeMouseModeActive) {
      _releaseCursorClip();
    }
  }

  void onWindowFocus() {
    if (!enabled.value) return;

    // macOS native mode: cursor is already locked
    if (_isNativeRelativeMouseModeActive) {
      setPointerInsideImage(false);
      return;
    }

    // Guard: image widget size must be available for proper center calculation.
    if (_imageWidgetSize == null) {
      _disableWithCleanup();
      return;
    }

    // Fail-safe: keep cursor usable on focus gain. Pointer lock will be re-engaged
    // on the next pointer enter/move/hover inside the remote image.
    setPointerInsideImage(false);
    _releaseCursorClip();

    // Best-effort: refresh center so the next engage is immediate.
    updatePointerLockCenter();
  }

  void toggleRelativeMouseMode() {
    final now = DateTime.now();
    if (_lastToggle != null &&
        now.difference(_lastToggle!).inMilliseconds <
            kRelativeMouseModeToggleDebounceMs) {
      return;
    }
    _lastToggle = now;
    setRelativeMouseMode(!enabled.value);
  }
}
