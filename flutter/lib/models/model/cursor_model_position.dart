part of 'model.dart';

extension CursorModelRect on CursorModel {
  // remote physical display coordinate
  // For update pan (mobile), onOneFingerPanStart, onOneFingerPanUpdate, onHoldDragUpdate
  Rect getVisibleRect() {
    final size = parent.target?.canvasModel.getSize() ??
        MediaQueryData.fromView(ui.window).size;
    final xoffset = parent.target?.canvasModel.x ?? 0;
    final yoffset = parent.target?.canvasModel.y ?? 0;
    final scale = parent.target?.canvasModel.scale ?? 1;
    final x0 = _displayOriginX - xoffset / scale;
    final y0 = _displayOriginY - yoffset / scale;
    return Rect.fromLTWH(x0, y0, size.width / scale, size.height / scale);
  }

  Offset getCanvasOffsetToCenterCursor() {
    // Cursor should be at the center of the visible rect.
    // _x = rect.left + rect.width / 2
    // _y = rect.right + rect.height / 2
    // See `getVisibleRect()`
    // _x = _displayOriginX - xoffset / scale + size.width / scale * 0.5;
    // _y = _displayOriginY - yoffset / scale + size.height / scale * 0.5;
    final size = parent.target?.canvasModel.getSize() ??
        MediaQueryData.fromView(ui.window).size;
    final xoffset = (_displayOriginX - _x) * scale + size.width * 0.5;
    final yoffset = (_displayOriginY - _y) * scale + size.height * 0.5;
    return Offset(xoffset, yoffset);
  }
}

extension CursorModelPosition on CursorModel {
  // mobile Soft keyboard, block touch event from the KeyHelpTools
  shouldBlock(double x, double y) {
    if (_blockEvents) {
      return true;
    }
    final offset = Offset(x, y);
    for (final rect in _blockedRects) {
      if (isPointInRect(offset, rect)) {
        return true;
      }
    }

    // For help tools rectangle, only block touch event when in touch mode.
    if (!(parent.target?.ffiModel.touchMode ?? false)) {
      return false;
    }
    if (_keyHelpToolsRect != null &&
        isPointInRect(offset, _keyHelpToolsRect!)) {
      return true;
    }
    return false;
  }

  // For touch mode
  Future<bool> move(double x, double y) async {
    if (shouldBlock(x, y)) {
      _lastIsBlocked = true;
      return false;
    }
    _lastIsBlocked = false;
    if (!_moveLocalIfInRemoteRect(x, y)) {
      return false;
    }
    await parent.target?.inputModel.moveMouse(_x, _y);
    return true;
  }

  Future<void> syncCursorPosition() async {
    await parent.target?.inputModel.moveMouse(_x, _y);
  }

  bool isInRemoteRect(Offset offset) {
    return getRemotePosInRect(offset) != null;
  }

  Offset? getRemotePosInRect(Offset offset) {
    final adjust = parent.target?.canvasModel.getAdjustY() ?? 0;
    final newPos = _getNewPos(offset.dx, offset.dy, adjust);
    final visibleRect = getVisibleRect();
    if (!isPointInRect(newPos, visibleRect)) {
      return null;
    }
    final rect = parent.target?.ffiModel.rect;
    if (rect != null) {
      if (!isPointInRect(newPos, rect)) {
        return null;
      }
    }
    return newPos;
  }

  Offset _getNewPos(double x, double y, double adjust) {
    final xoffset = parent.target?.canvasModel.x ?? 0;
    final yoffset = parent.target?.canvasModel.y ?? 0;
    final newX = (x - xoffset) / scale + _displayOriginX;
    final newY = (y - yoffset - adjust) / scale + _displayOriginY;
    return Offset(newX, newY);
  }

  bool _moveLocalIfInRemoteRect(double x, double y) {
    final newPos = getRemotePosInRect(Offset(x, y));
    if (newPos == null) {
      return false;
    }
    _x = newPos.dx;
    _y = newPos.dy;
    _notify();
    return true;
  }

  moveLocal(double x, double y, {double adjust = 0}) {
    final newPos = _getNewPos(x, y, adjust);
    _x = newPos.dx;
    _y = newPos.dy;
    _notify();
  }

  reset() {
    _x = _displayOriginX;
    _y = _displayOriginY;
    parent.target?.inputModel.moveMouse(_x, _y);
    parent.target?.canvasModel.reset();
    _notify();
  }
}
