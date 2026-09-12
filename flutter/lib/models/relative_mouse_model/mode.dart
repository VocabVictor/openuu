part of 'relative_mouse_model.dart';

extension RelativeMouseMode on RelativeMouseModel {
  bool setRelativeMouseMode(bool value) {
    // Web is not supported due to Pointer Lock API integration complexity with Flutter's input system
    if (isWeb) {
      return false;
    }

    if (value) {
      if (!keyboardPerm() || isViewCamera()) {
        return false;
      }

      if (isDesktop && _imageWidgetSize == null) {
        // Desktop only: Ensure image widget size is available for proper center calculation.
        showToast(translate('rel-mouse-not-ready-tip'));
        return false;
      }

      if (!isSupported) {
        // Check server version support before enabling.
        showToast(translate('rel-mouse-not-supported-peer-tip'));
        return false;
      }
    }

    if (value) {
      try {
        if (isDesktop) {
          final requestId = ++_enableRequestId;
          if (isMacOS) {
            // macOS: Use native relative mouse mode with CGAssociateMouseAndMouseCursorPosition
            // This locks the cursor in place and provides raw delta via NSEvent monitor.
            _enableNativeRelativeMouseMode().then((success) {
              // Guard against stale callback: user may have toggled off relative mode
              // while the async enable was in progress.
              if (_enableRequestId != requestId) {
                return;
              }
              if (success) {
                _completeEnableRelativeMouseMode();
              }
              // Note: _enableNativeRelativeMouseMode already handles its own cleanup on failure
            });
          } else {
            // Windows/Linux: Use Flutter-based cursor recenter approach
            if (!getPointerInsideImage()) {
              _releaseCursorClip();
            }

            updatePointerLockCenter().then((_) => _recenterMouse()).then((_) {
              if (_enableRequestId != requestId) {
                return;
              }
              _completeEnableRelativeMouseMode();
            }).catchError((e) {
              if (_enableRequestId != requestId) {
                return;
              }
              debugPrint('[RelMouse] Platform setup failed: $e');
              _resetState();
            });
          }
        } else {
          // Mobile: enable immediately (no platform-specific setup needed)
          _completeEnableRelativeMouseMode();
        }
      } catch (e) {
        _disableWithCleanup();
        return false;
      }
    } else {
      // Best-effort marker for Rust rdev grab loop (ESC behavior).
      // Bypass keyboardPerm check to ensure Rust state is always synced,
      // even if permission was revoked while relative mode was active.
      _sendMouseMessageToSession(
        {
          'relative_mouse_mode': '0',
        },
        disableRelativeOnError: false,
        bypassKeyboardPerm: true,
      );

      // Desktop only: cursor manipulation
      if (isDesktop) {
        if (isMacOS) {
          // macOS: Disable native relative mouse mode
          // This already calls CGAssociateMouseAndMouseCursorPosition(1) to re-associate mouse
          _disableNativeRelativeMouseMode();
        } else {
          _releaseCursorClip();
        }
      }
      enabled.value = false;
      _resetState();
      onDisabled?.call();
    }

    return true;
  }

  /// Called when platform setup completes successfully to finalize enabling relative mouse mode.
  void _completeEnableRelativeMouseMode() {
    enabled.value = true;

    // Show toast notification so user knows how to exit relative mouse mode (desktop only).
    if (isDesktop) {
      showToast(
          translate('rel-mouse-exit-{${isMacOS ? "Cmd+G" : "Ctrl+Alt"}}-tip'),
          alignment: Alignment.center);
    }

    // Best-effort marker for Rust rdev grab loop (ESC behavior) and peer/server state.
    // This uses a no-op delta so it does not move the remote cursor.
    // Intentionally fire-and-forget: we don't block enabling on this marker message.
    // Failures are logged but do not disable relative mouse mode.
    _sendMouseMessageToSession(
      {
        'relative_mouse_mode': '1',
        'type': 'move_relative',
        'x': '0',
        'y': '0',
      },
      disableRelativeOnError: false,
    ).catchError((e) {
      debugPrint('[RelMouse] Failed to send enable marker: $e');
      return false;
    });
  }
}
