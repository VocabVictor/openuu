part of 'input_model.dart';

extension InputModelTrackpad on InputModel {
  void onPointHoverImage(PointerHoverEvent e) {
    _stopFling = true;
    if (isViewOnly && !showMyCursor) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;

    // May fix https://github.com/rustdesk/rustdesk/issues/13009
    if (isIOS && e.synthesized && e.position == Offset.zero && e.buttons == 0) {
      // iOS may emit a synthesized hover event at (0,0) when the mouse is disconnected.
      // Ignore this event to prevent cursor jumping.
      debugPrint('Ignored synthesized hover at (0,0) on iOS');
      return;
    }

    // Only update pointer region when relative mouse mode is enabled.
    // This avoids unnecessary tracking when not in relative mode.
    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (!isPhysicalMouse.value) {
      isPhysicalMouse.value = true;
    }
    if (isPhysicalMouse.value) {
      if (!_relativeMouse.handleRelativeMouseMove(e.localPosition)) {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventMove), canvasPosition,
            edgeScroll: useEdgeScroll);
      }
    }
  }

  void onPointerPanZoomStart(PointerPanZoomStartEvent e) {
    _lastScale = 1.0;
    _stopFling = true;
    if (isViewOnly) return;
    if (isViewCamera) return;
    if (peerPlatform == kPeerPlatformAndroid) {
      handlePointerEvent('touch', kMouseEventTypePanStart, e.position);
    }
  }

  // https://docs.flutter.dev/release/breaking-changes/trackpad-gestures
  void onPointerPanZoomUpdate(PointerPanZoomUpdateEvent e) {
    if (isViewOnly) return;
    if (isViewCamera) return;
    if (peerPlatform != kPeerPlatformAndroid) {
      final scale = ((e.scale - _lastScale) * 1000).toInt();
      _lastScale = e.scale;

      if (scale != 0) {
        bind.sessionSendPointer(
            sessionId: sessionId,
            msg: json.encode(
                PointerEventToRust(kPointerEventKindTouch, 'scale', scale)
                    .toJson()));
        return;
      }
    }

    var delta = e.panDelta * _trackpadSpeedInner;
    if (isMacOS && peerPlatform == kPeerPlatformWindows) {
      delta *= _trackpadAdjustMacToWin;
    }
    delta = _filterTrackpadDeltaAxis(delta);
    _trackpadLastDelta = delta;

    var x = delta.dx.toInt();
    var y = delta.dy.toInt();
    if (peerPlatform == kPeerPlatformLinux) {
      _trackpadScrollUnsent += (delta * _trackpadAdjustPeerLinux);
      x = _trackpadScrollUnsent.dx.truncate();
      y = _trackpadScrollUnsent.dy.truncate();
      _trackpadScrollUnsent -= Offset(x.toDouble(), y.toDouble());
    } else {
      if (x == 0 && y == 0) {
        final thr = 0.1;
        if (delta.dx.abs() > delta.dy.abs()) {
          x = delta.dx > thr ? 1 : (delta.dx < -thr ? -1 : 0);
        } else {
          y = delta.dy > thr ? 1 : (delta.dy < -thr ? -1 : 0);
        }
      }
    }
    if (x != 0 || y != 0) {
      if (peerPlatform == kPeerPlatformAndroid) {
        handlePointerEvent('touch', kMouseEventTypePanUpdate,
            Offset(x.toDouble(), y.toDouble()));
      } else {
        if (isViewCamera) return;
        bind.sessionSendMouse(
            sessionId: sessionId,
            msg: '{"type": "trackpad", "x": "$x", "y": "$y"}');
      }
    }
  }

  Offset _filterTrackpadDeltaAxis(Offset delta) {
    final absDx = delta.dx.abs();
    final absDy = delta.dy.abs();
    // Keep diagonal intent when movement is tiny on both axes.
    if (absDx < InputModel._trackpadAxisNoiseThreshold &&
        absDy < InputModel._trackpadAxisNoiseThreshold) {
      return delta;
    }
    // Dominant-axis lock to reduce accidental cross-axis scrolling noise.
    if (absDy >= absDx * InputModel._trackpadAxisLockRatio) {
      return Offset(0, delta.dy);
    }
    if (absDx >= absDy * InputModel._trackpadAxisLockRatio) {
      return Offset(delta.dx, 0);
    }
    return delta;
  }

  void _scheduleFling(double x, double y, int delay) {
    if (isViewCamera) return;
    if ((x == 0 && y == 0) || _stopFling) {
      _fling = false;
      return;
    }

    _flingTimer = Timer(Duration(milliseconds: delay), () {
      if (_stopFling) {
        _fling = false;
        return;
      }

      final d = 0.97;
      x *= d;
      y *= d;

      // Try set delta (x,y) and delay.
      var dx = x.toInt();
      var dy = y.toInt();
      if (parent.target?.ffiModel.pi.platform == kPeerPlatformLinux) {
        dx = (x * _trackpadAdjustPeerLinux).toInt();
        dy = (y * _trackpadAdjustPeerLinux).toInt();
      }

      var delay = _flingBaseDelay;

      if (dx == 0 && dy == 0) {
        _fling = false;
        return;
      }

      bind.sessionSendMouse(
          sessionId: sessionId,
          msg: '{"type": "trackpad", "x": "$dx", "y": "$dy"}');
      _scheduleFling(x, y, delay);
    });
  }

  void waitLastFlingDone() {
    if (_fling) {
      _stopFling = true;
    }
    for (var i = 0; i < 5; i++) {
      if (!_fling) {
        break;
      }
      sleep(Duration(milliseconds: 10));
    }
    _flingTimer?.cancel();
  }

  void onPointerPanZoomEnd(PointerPanZoomEndEvent e) {
    if (isViewCamera) return;
    if (peerPlatform == kPeerPlatformAndroid) {
      handlePointerEvent('touch', kMouseEventTypePanEnd, e.position);
      return;
    }

    bind.sessionSendPointer(
        sessionId: sessionId,
        msg: json.encode(
            PointerEventToRust(kPointerEventKindTouch, 'scale', 0).toJson()));

    waitLastFlingDone();
    _stopFling = false;

    // 2.0 is an experience value
    double minFlingValue = 2.0 * _trackpadSpeedInner;
    if (isMacOS && peerPlatform == kPeerPlatformWindows) {
      minFlingValue *= _trackpadAdjustMacToWin;
    }
    if (_trackpadLastDelta.dx.abs() > minFlingValue ||
        _trackpadLastDelta.dy.abs() > minFlingValue) {
      _fling = true;
      _scheduleFling(
          _trackpadLastDelta.dx, _trackpadLastDelta.dy, _flingBaseDelay);
    }
    _trackpadLastDelta = Offset.zero;
  }
}
