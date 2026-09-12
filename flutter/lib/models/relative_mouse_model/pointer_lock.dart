part of 'relative_mouse_model.dart';

extension RelativeMousePointerLock on RelativeMouseModel {
  /// Recenter the cursor to the pointer lock center.
  /// Fire-and-forget safe: prevents overlapping calls and catches errors internally.
  Future<void> _recenterMouse() async {
    // Prevent overlapping recenter operations under high-frequency mouse moves.
    if (_recenterInProgress) return;
    _recenterInProgress = true;

    try {
      if (!enabled.value) return;
      if (!getPointerInsideImage()) return;

      final center = _pointerLockCenterScreen;
      if (center == null) {
        return;
      }

      for (int attempt = 0; attempt < RelativeMouseModel._recenterMaxRetries; attempt++) {
        // Check preconditions before each attempt.
        if (!enabled.value || !getPointerInsideImage()) return;

        final ok = bind.mainSetCursorPosition(
          x: center.dx.toInt(),
          y: center.dy.toInt(),
        );
        if (ok) {
          // Skip the next mouse move event - it's triggered by the recenter itself.
          _skipNextMouseMove = true;
          return;
        }

        // Wait before retrying (except on the last attempt).
        if (attempt < RelativeMouseModel._recenterMaxRetries - 1) {
          await Future.delayed(RelativeMouseModel._recenterRetryDelay);
        }
      }

      // All attempts failed.
      _disableWithCleanup();
      showToast(translate('rel-mouse-lock-failed-tip'));
    } catch (e, st) {
      debugPrint('[RelMouse] Unexpected error in _recenterMouse: $e\n$st');
    } finally {
      _recenterInProgress = false;
    }
  }

  Future<void> updatePointerLockCenter({Offset? localCenter}) async {
    if (!isDesktop) return;

    // Null safety check for kWindowId.
    if (kWindowId == null) {
      if (enabled.value) {
        _disableWithCleanup();
      }
      return;
    }

    try {
      final wc = WindowController.fromWindowId(kWindowId!);
      final frame = await wc.getFrame();

      if (frame.width <= 0 || frame.height <= 0) {
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }

      if (localCenter != null) {
        _pointerLockCenterLocal = localCenter;
      } else if (_imageWidgetSize != null) {
        _pointerLockCenterLocal = Offset(
          _imageWidgetSize!.width / 2,
          _imageWidgetSize!.height / 2,
        );
      } else {
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }

      // Calculate screen coordinates for OS cursor positioning.
      // Use PlatformDispatcher instead of deprecated ui.window.
      final view = ui.PlatformDispatcher.instance.views.firstOrNull;
      if (view == null) {
        debugPrint('[RelMouse] No view available for coordinate calculation');
        if (enabled.value) {
          _disableWithCleanup();
        }
        return;
      }
      final scale = view.devicePixelRatio;

      if (_pointerRegionTopLeftGlobal != null && scale > 0) {
        // On macOS, window frame and CGWarpMouseCursorPosition use points (not pixels).
        // On Windows, they use pixels.
        // Flutter's logical coordinates are in points on macOS.
        final centerInView =
            _pointerRegionTopLeftGlobal! + _pointerLockCenterLocal!;

        // Calculate client area offset (excluding title bar and borders)
        final clientPhysical = view.physicalSize;

        // macOS: Window frame and CGWarpMouseCursorPosition both use points (not pixels).
        // We convert clientPhysical (pixels) to points via `/ scale` to compute titleBarHeight,
        // which is the difference between the total window height and the Flutter view height.
        if (isMacOS) {
          final clientHeightPoints = clientPhysical.height / scale;
          final titleBarHeight = frame.height - clientHeightPoints;

          _pointerLockCenterScreen = Offset(
            frame.left + centerInView.dx,
            frame.top + titleBarHeight + centerInView.dy,
          );
        } else {
          // Windows/Linux: Use pixel coordinates. We estimate the client-area offset using
          // a heuristic based on the difference between frame size and client physical size.
          // This assumes symmetric horizontal borders (extraW / 2) and that the remaining
          // vertical space (extraH - borderBottom) is the title bar height.
          // Limitation: This heuristic may be inaccurate for maximized windows, custom window
          // decorations, or when the OS uses different border styles.
          // TODO: Replace this heuristic with platform API calls (e.g., GetClientRect on Windows)
          //       if precise client-area offsets are required.
          final extraW = frame.width - clientPhysical.width;
          final extraH = frame.height - clientPhysical.height;
          final borderX = extraW > 0 ? extraW / 2 : 0.0;
          final borderBottom = borderX;
          final borderTop = extraH > borderBottom ? extraH - borderBottom : 0.0;
          final clientTopLeftScreen =
              Offset(frame.left + borderX, frame.top + borderTop);

          // Calculate tentative center, then validate it's within frame bounds.
          // This guards against heuristic inaccuracies (e.g., maximized windows).
          final tentativeCenter = Offset(
            clientTopLeftScreen.dx + centerInView.dx * scale,
            clientTopLeftScreen.dy + centerInView.dy * scale,
          );
          final withinFrame = tentativeCenter.dx >= frame.left &&
              tentativeCenter.dx <= frame.left + frame.width &&
              tentativeCenter.dy >= frame.top &&
              tentativeCenter.dy <= frame.top + frame.height;
          _pointerLockCenterScreen = withinFrame
              ? tentativeCenter
              : Offset(
                  frame.left + frame.width / 2, frame.top + frame.height / 2);
        }
      } else {
        _pointerLockCenterScreen = Offset(
          frame.left + frame.width / 2,
          frame.top + frame.height / 2,
        );
      }

      if (enabled.value && isWindows && getPointerInsideImage()) {
        _applyCursorClipForFrame(frame);
      } else if (enabled.value && isWindows && _cursorClipApplied) {
        // Only release if we actually have a clip applied to avoid redundant FFI calls.
        _releaseCursorClip();
      }
      // macOS: no clip_cursor (CGAssociateMouseAndMouseCursorPosition stops mouse events)
      // Instead, we use recenter method like other platforms.
    } catch (e) {
      if (enabled.value) {
        _disableWithCleanup();
      } else {
        _pointerLockCenterLocal = null;
        _pointerLockCenterScreen = null;
      }
    }
  }

  void _ensurePointerLockEngaged() {
    if (!enabled.value) return;
    if (!isDesktop) return;

    setPointerInsideImage(true);

    final needsCenter =
        _pointerLockCenterLocal == null || _pointerLockCenterScreen == null;
    // Windows only: cursor clip
    final needsClip = isWindows && !_cursorClipApplied;
    if (needsCenter || needsClip) {
      updatePointerLockCenter()
          .then((_) => _recenterMouse())
          .catchError((Object e, StackTrace st) {
        debugPrint('[RelMouse] updatePointerLockCenter failed: $e\n$st');
        _disableWithCleanup();
      });
    }
  }
}
