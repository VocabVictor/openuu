part of 'input_model.dart';

extension InputModelSignal on InputModel {
  /// Handle scroll/wheel events.
  /// Note: Scroll events intentionally use absolute positioning even in relative mouse mode.
  /// This is because scroll events don't need relative positioning - they represent
  /// scroll deltas that are independent of cursor position. Games and 3D applications
  /// handle scroll events the same way regardless of mouse mode.
  void onPointerSignalImage(PointerSignalEvent e) {
    if (isViewOnly) return;
    if (isViewCamera) return;
    if (e is PointerScrollEvent) {
      final rawDx = e.scrollDelta.dx;
      final rawDy = e.scrollDelta.dy;
      final dominantDelta = rawDx.abs() > rawDy.abs() ? rawDx.abs() : rawDy.abs();
      final isSmooth = dominantDelta < 1;
      final nowUs = DateTime.now().microsecondsSinceEpoch;
      final dtUs = _lastWheelTsUs == 0 ? 0 : nowUs - _lastWheelTsUs;
      _lastWheelTsUs = nowUs;
      int accel = 1;
      if (!isSmooth &&
          dtUs > 0 &&
          dtUs <= InputModel._wheelAccelMediumThresholdUs &&
          (isWindows || isLinux) &&
          peerPlatform == kPeerPlatformMacOS) {
        final velocity = dominantDelta / dtUs;
        if (velocity >= InputModel._wheelBurstVelocityThreshold) {
          if (dtUs < InputModel._wheelAccelFastThresholdUs) {
            accel = 3;
          } else {
            accel = 2;
          }
        }
      }
      var dx = rawDx.toInt();
      var dy = rawDy.toInt();
      if (rawDx.abs() > rawDy.abs()) {
        dy = 0;
      } else {
        dx = 0;
      }
      if (dx > 0) {
        dx = -accel;
      } else if (dx < 0) {
        dx = accel;
      }
      if (dy > 0) {
        dy = -accel;
      } else if (dy < 0) {
        dy = accel;
      }
      bind.sessionSendMouse(
          sessionId: sessionId,
          msg: '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }
  }

  void refreshMousePos() => handleMouse({
        'buttons': 0,
        'type': _kMouseEventMove,
      }, lastMousePos, edgeScroll: useEdgeScroll);

  void tryMoveEdgeOnExit(Offset pos) => handleMouse(
        {
          'buttons': 0,
          'type': _kMouseEventMove,
        },
        pos,
        onExit: true,
      );
}

extension InputModelTouch on InputModel {
  /// Check if a touch tap event should be ignored because it's a duplicate
  /// of a recent mouse event (iOS Magic Mouse issue).
  bool shouldIgnoreTouchTap(ui.Offset pos) {
    if (!isIOS) return false;
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    final dt = nowMs - _lastMouseDownTimeMs;
    final distance = (_lastMouseDownPos - pos).distance;
    // If touch tap is within 2000ms and 80px of the last mouse down,
    // it's likely a duplicate event from the same Magic Mouse click.
    if (dt >= 0 && dt < 2000 && distance < 80.0) {
      debugPrint("shouldIgnoreTouchTap: IGNORED (dt=$dt, dist=$distance)");
      return true;
    }
    return false;
  }

  /// iOS may emit a synthesized touch event after a real mouse click.
  /// This helper ignores touch-down events that arrive shortly after a mouse down,
  /// even when the position is far (e.g., near the top edge).
  bool _shouldIgnoreTouchAfterMouse(int nowMs) {
    if (!isIOS) return false;
    const int kTouchAfterMouseWindowMs = 700;
    final dt = nowMs - _lastMouseDownTimeMs;
    return dt >= 0 && dt < kTouchAfterMouseWindowMs;
  }

  void onPointDownImage(PointerDownEvent e) {
    debugPrint("onPointDownImage ${e.kind}");
    _stopFling = true;
    if (isDesktop) _queryOtherWindowCoords = true;
    _remoteWindowCoords = [];
    _windowRect = null;
    if (isViewOnly && !showMyCursor) return;
    if (isViewCamera) return;

    // Track mouse down events for duplicate detection on iOS.
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    if (e.kind == ui.PointerDeviceKind.mouse) {
      if (!isPhysicalMouse.value) {
        isPhysicalMouse.value = true;
      }
      _lastMouseDownTimeMs = nowMs;
      _lastMouseDownPos = e.position;
    }

    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (e.kind != ui.PointerDeviceKind.mouse) {
      // Ignore duplicate touch events that follow a recent mouse click (iOS Magic Mouse issue).
      if (isPhysicalMouse.value && _shouldIgnoreTouchAfterMouse(nowMs)) {
        return;
      }
      if (isPhysicalMouse.value) {
        isPhysicalMouse.value = false;
      }
    }
    if (isPhysicalMouse.value) {
      // In relative mouse mode, send button events without position.
      // Use _relativeMouse.enabled.value consistently with the guard above.
      if (_relativeMouse.enabled.value) {
        _relativeMouse
            .sendRelativeMouseButton(_getMouseEvent(e, _kMouseEventDown));
      } else {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventDown), canvasPosition);
      }
    }
  }

  void onPointUpImage(PointerUpEvent e) {
    if (isDesktop) _queryOtherWindowCoords = false;
    if (isViewOnly && !showMyCursor) return;
    if (isViewCamera) return;

    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (isPhysicalMouse.value) {
      // In relative mouse mode, send button events without position.
      // Use _relativeMouse.enabled.value consistently with the guard above.
      if (_relativeMouse.enabled.value) {
        _relativeMouse
            .sendRelativeMouseButton(_getMouseEvent(e, _kMouseEventUp));
      } else {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventUp), canvasPosition);
      }
    }
  }

  void onPointMoveImage(PointerMoveEvent e) {
    if (isViewOnly && !showMyCursor) return;
    if (isViewCamera) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;

    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (_queryOtherWindowCoords) {
      Future.delayed(Duration.zero, () async {
        _windowRect = await InputModel.fillRemoteCoordsAndGetCurFrame(_remoteWindowCoords);
      });
      _queryOtherWindowCoords = false;
    }
    if (isPhysicalMouse.value) {
      if (!_relativeMouse.handleRelativeMouseMove(e.localPosition)) {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventMove), canvasPosition,
            edgeScroll: useEdgeScroll);
      }
    }
  }

  /// Convert pointer coordinates into the visible remote canvas space.
  ///
  /// On mobile, the remote page body is wrapped in `SafeArea`, but the pointer
  /// listener that feeds these events sits outside that subtree. As a result,
  /// `event.localPosition` still includes the top/left safe-area inset.
  ///
  /// When the keyboard-visible path shows `KeyHelpTools`, the remote canvas is
  /// also shifted downward by `CanvasModel.getAdjustY()`. The downstream mouse
  /// mapping logic expects coordinates relative to the visible canvas area, so
  /// we subtract both the mobile safe-area padding and the current canvas
  /// adjustment before passing the position into mouse mapping.
  ///
  /// Desktop and web desktop continue to use the global position directly
  /// because their pointer mapping is window-based.
  Offset _pointerPositionForRemoteCanvas(PointerEvent event) {
    if (isDesktop) {
      return event.position;
    }
    final mediaData = MediaQueryData.fromView(
        WidgetsBinding.instance.platformDispatcher.views.first);
    final adjustY = parent.target?.canvasModel.getAdjustY() ?? 0.0;
    return Offset(
      event.localPosition.dx - mediaData.padding.left,
      event.localPosition.dy - mediaData.padding.top - adjustY,
    );
  }
}
