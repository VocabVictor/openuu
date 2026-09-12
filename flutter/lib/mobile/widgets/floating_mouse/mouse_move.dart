part of 'floating_mouse.dart';

extension _FloatingMouseMove on _FloatingMouseState {
  // If the mouse is very close to the edge of the display,
  // we can only start auto scroll when the mouse is at the edge of the screen.
  bool _shouldAutoScrollIfCursorNearRemoteEdge(double remoteEdge,
      double remoteValue, double localEdge, double localValue) {
    if ((remoteEdge - remoteValue).abs() < 100.0) {
      if (!_isValueAtEdge(localEdge, localValue)) {
        return false;
      }
    }
    return true;
  }

  void _onMoveUpdateDelta(Offset delta) {
    _resetCollapseTimer();
    final context = this.context;
    final size = MediaQuery.of(context).size;
    Offset newPosition = _position + delta;
    double minX = 0;
    double minY = 0;
    double maxX = size.width - mouseWidth;
    double maxY = size.height - mouseHeight;
    newPosition = Offset(
      newPosition.dx.clamp(minX, maxX),
      newPosition.dy.clamp(minY, maxY),
    );
    _setState(() {
      final isPositionChanged = !(isDoubleEqual(newPosition.dx, _position.dx) &&
          isDoubleEqual(newPosition.dy, _position.dy));
      _position = newPosition;
      if (!_isExpanded) {
        return;
      }

      Offset? mouseGlobalPosition;
      Offset? positionInRemoteDisplay;
      if (isPositionChanged) {
        mouseGlobalPosition = _getMouseGlobalPosition();
        final evt = _inputModel.handleMouse(
            InputModel.getMouseEventMove(), mouseGlobalPosition,
            moveCanvas: false);
        positionInRemoteDisplay = _FloatingMouseState._getPositionFromMouseRetEvt(evt);
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) _updateBlockedRect();
        });
      }

      // Get the display rect
      final displayRect = widget.ffi.ffiModel.displaysRect();
      if (displayRect == null) {
        _canvasScrollState.tryCancel();
        return;
      }

      // Get the mouse global position and position in remote display
      mouseGlobalPosition ??= _getMouseGlobalPosition();
      if (positionInRemoteDisplay == null) {
        final evt = _inputModel.processEventToPeer(
            InputModel.getMouseEventMove(), mouseGlobalPosition,
            moveCanvas: false);
        positionInRemoteDisplay = _FloatingMouseState._getPositionFromMouseRetEvt(evt);
      }

      // Check if need to start auto canvas scroll
      // If:
      // 1. The mouse is near the edge of the screen.
      // 2. The position in remote display is in the rect of the display.
      // 3. If the remote cursor is near the edge of the remote display,
      //    then the local mouse must be at the edge of the screen.
      // Then start auto canvas scroll.
      if (_isValueNearEdge(minX, _position.dx)) {
        bool shouldStartScroll = true;
        if (_isValueAtOrOutsideEdge(
            displayRect.left, positionInRemoteDisplay?.dx)) {
          shouldStartScroll = false;
        }
        if (positionInRemoteDisplay != null) {
          if (!_shouldAutoScrollIfCursorNearRemoteEdge(displayRect.left,
              positionInRemoteDisplay.dx, minX, _position.dx)) {
            shouldStartScroll = false;
          }
        }
        if (!shouldStartScroll) {
          _canvasScrollState.tryCancel();
          return;
        }
        _canvasScrollState.scrollX = 1.0 * _CanvasScrollState.speedPressed;
      } else if (_isValueNearEdge(minY, _position.dy)) {
        bool shouldStartScroll = true;
        if (_isValueAtOrOutsideEdge(
            displayRect.top, positionInRemoteDisplay?.dy)) {
          shouldStartScroll = false;
        }
        if (positionInRemoteDisplay != null) {
          if (!_shouldAutoScrollIfCursorNearRemoteEdge(displayRect.top,
              positionInRemoteDisplay.dy, minY, _position.dy)) {
            shouldStartScroll = false;
          }
        }
        if (!shouldStartScroll) {
          _canvasScrollState.tryCancel();
          return;
        }
        _canvasScrollState.scrollY = 1.0 * _CanvasScrollState.speedPressed;
      } else if (_isValueNearEdge(maxX, _position.dx)) {
        bool shouldStartScroll = true;
        if (_isValueAtOrOutsideEdge(
            displayRect.right - 1, positionInRemoteDisplay?.dx)) {
          shouldStartScroll = false;
        }
        if (positionInRemoteDisplay != null) {
          if (!_shouldAutoScrollIfCursorNearRemoteEdge(displayRect.right - 1,
              positionInRemoteDisplay.dx, maxX, _position.dx)) {
            shouldStartScroll = false;
          }
        }
        if (!shouldStartScroll) {
          _canvasScrollState.tryCancel();
          return;
        }
        _canvasScrollState.scrollX = -1.0 * _CanvasScrollState.speedPressed;
      } else if (_isValueNearEdge(maxY, _position.dy)) {
        bool shouldStartScroll = true;
        if (_isValueAtOrOutsideEdge(
            displayRect.bottom - 1, positionInRemoteDisplay?.dy)) {
          shouldStartScroll = false;
        }
        if (positionInRemoteDisplay != null) {
          if (!_shouldAutoScrollIfCursorNearRemoteEdge(displayRect.bottom - 1,
              positionInRemoteDisplay.dy, maxY, _position.dy)) {
            shouldStartScroll = false;
          }
        }
        if (!shouldStartScroll) {
          _canvasScrollState.tryCancel();
          return;
        }
        _canvasScrollState.scrollY = -1.0 * _CanvasScrollState.speedPressed;
      } else {
        _canvasScrollState.tryCancel();
        return;
      }
      _canvasScrollState.tryStart(displayRect, mouseGlobalPosition);
    });
  }

  void _onDragHandleUpdate(DragUpdateDetails details) =>
      _onMoveUpdateDelta(details.delta);

  void _onBodyPointerMoveUpdate(PointerMoveEvent event) =>
      _onMoveUpdateDelta(event.delta);

  bool _containsPosition(GlobalKey key, Offset pos) {
    final contextScroll = key.currentContext;
    if (contextScroll == null) return false;
    final RenderBox? scrollWheelBox =
        contextScroll.findRenderObject() as RenderBox?;
    if (scrollWheelBox == null || !scrollWheelBox.attached) return false;
    Rect rect = scrollWheelBox.localToGlobal(Offset.zero) & scrollWheelBox.size;
    return rect.contains(pos);
  }

  void _handlePointerDown(PointerDownEvent event) {
    _resetCollapseTimer();
    if (_isScrolling) return;
    if (_containsPosition(_scrollWheelUpKey, event.position) ||
        _containsPosition(_scrollWheelDownKey, event.position)) {
      final contextMouse = _mouseWidgetKey.currentContext;
      if (contextMouse == null) return;
      final RenderBox? mouseBox = contextMouse.findRenderObject() as RenderBox?;
      if (mouseBox == null || !mouseBox.attached) return;

      // Only enter scroll mode when all RenderObjects are available.
      final Offset mouseTopLeft = mouseBox.localToGlobal(Offset.zero);
      final Size mouseSize = mouseBox.size;
      final Offset center =
          mouseTopLeft + Offset(mouseSize.width / 2, mouseSize.height / 2);

      final vector = event.position - center;
      final rawAngle = atan2(vector.dy, vector.dx);

      final closestDotIndex = (rawAngle / _kDotAngle).round();
      _lastSnappedAngle = closestDotIndex * _kDotAngle;

      _setState(() {
        _isScrolling = true;
        _cursorModel.blockEvents = true;
        _scrollCenter = center;
        _snappedPointerAngle = _lastSnappedAngle!;
      });
    }
  }

  void _handlePointerMove(PointerMoveEvent event) {
    _resetCollapseTimer();
    if (!_isScrolling || _scrollCenter == null || _lastSnappedAngle == null) {
      return;
    }

    final touchPosition = event.position;
    final vector = touchPosition - _scrollCenter!;
    final rawCurrentAngle = atan2(vector.dy, vector.dx);

    final closestDotIndex = (rawCurrentAngle / _kDotAngle).round();
    final snappedCurrentAngle = closestDotIndex * _kDotAngle;

    if (snappedCurrentAngle == _lastSnappedAngle) return;

    double deltaAngle = snappedCurrentAngle - _lastSnappedAngle!;

    if (deltaAngle.abs() > pi) {
      deltaAngle = (deltaAngle > 0) ? deltaAngle - 2 * pi : deltaAngle + 2 * pi;
    }

    _lastSnappedAngle = snappedCurrentAngle;

    _setState(() {
      _snappedPointerAngle = snappedCurrentAngle;
      _inputModel.scroll(deltaAngle > 0 ? -1 : 1);
    });
  }

  void _tryCancelScrolling() {
    _resetCollapseTimer();
    if (!_isScrolling) return;
    _setState(() {
      _isScrolling = false;
      _cursorModel.blockEvents = false;
      _lastSnappedAngle = null;
      _scrollCenter = null;
    });
  }
}
