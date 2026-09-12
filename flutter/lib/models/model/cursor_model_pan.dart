part of 'model.dart';

extension CursorModelPan on CursorModel {
  updatePan(Offset delta, Offset localPosition, bool touchMode) async {
    if (touchMode) {
      await _handleTouchMode(delta, localPosition);
      return;
    }
    double dx = delta.dx;
    double dy = delta.dy;
    if (parent.target?.imageModel.image == null) return;
    final scale = parent.target?.canvasModel.scale ?? 1.0;
    dx /= scale;
    dy /= scale;
    final r = getVisibleRect();
    var cx = r.center.dx;
    var cy = r.center.dy;
    var tryMoveCanvasX = false;
    final displayRect = parent.target?.ffiModel.rect;
    if (dx > 0) {
      final maxCanvasCanMove = _displayOriginX +
          (displayRect?.width ?? 1280) -
          r.right.roundToDouble();
      tryMoveCanvasX = _x + dx > cx && maxCanvasCanMove > 0;
      if (tryMoveCanvasX) {
        dx = min(dx, maxCanvasCanMove);
      } else {
        final maxCursorCanMove = r.right - _x;
        dx = min(dx, maxCursorCanMove);
      }
    } else if (dx < 0) {
      final maxCanvasCanMove = _displayOriginX - r.left.roundToDouble();
      tryMoveCanvasX = _x + dx < cx && maxCanvasCanMove < 0;
      if (tryMoveCanvasX) {
        dx = max(dx, maxCanvasCanMove);
      } else {
        final maxCursorCanMove = r.left - _x;
        dx = max(dx, maxCursorCanMove);
      }
    }
    var tryMoveCanvasY = false;
    if (dy > 0) {
      final mayCanvasCanMove = _displayOriginY +
          (displayRect?.height ?? 720) -
          r.bottom.roundToDouble();
      tryMoveCanvasY = _y + dy > cy && mayCanvasCanMove > 0;
      if (tryMoveCanvasY) {
        dy = min(dy, mayCanvasCanMove);
      } else {
        final mayCursorCanMove = r.bottom - _y;
        dy = min(dy, mayCursorCanMove);
      }
    } else if (dy < 0) {
      final mayCanvasCanMove = _displayOriginY - r.top.roundToDouble();
      tryMoveCanvasY = _y + dy < cy && mayCanvasCanMove < 0;
      if (tryMoveCanvasY) {
        dy = max(dy, mayCanvasCanMove);
      } else {
        final mayCursorCanMove = r.top - _y;
        dy = max(dy, mayCursorCanMove);
      }
    }

    if (dx == 0 && dy == 0) return;

    Point<double>? newPos;
    final rect = parent.target?.ffiModel.rect;
    if (rect == null) {
      // unreachable
      return;
    }
    newPos = InputModel.getPointInRemoteRect(
        false,
        parent.target?.ffiModel.pi.platform,
        kPointerEventKindMouse,
        kMouseEventTypeDefault,
        _x + dx,
        _y + dy,
        rect,
        buttons: kPrimaryButton);
    if (newPos == null) {
      return;
    }
    dx = newPos.x - _x;
    dy = newPos.y - _y;
    _x = newPos.x;
    _y = newPos.y;
    if (tryMoveCanvasX && dx != 0) {
      parent.target?.canvasModel.panX(-dx * scale);
    }
    if (tryMoveCanvasY && dy != 0) {
      parent.target?.canvasModel.panY(-dy * scale);
    }

    parent.target?.inputModel.moveMouse(_x, _y);
    _notify();
  }

  bool _isInCurrentWindow(double x, double y) {
    final w = _windowRect!.width / devicePixelRatio;
    final h = _windowRect!.width / devicePixelRatio;
    return x >= 0 && y >= 0 && x <= w && y <= h;
  }

  _handleTouchMode(Offset delta, Offset localPosition) async {
    bool isMoved = false;
    if (_remoteWindowCoords.isNotEmpty &&
        _windowRect != null &&
        !_isInCurrentWindow(localPosition.dx, localPosition.dy)) {
      final coords = InputModel.findRemoteCoords(localPosition.dx,
          localPosition.dy, _remoteWindowCoords, devicePixelRatio);
      if (coords != null) {
        double x2 =
            (localPosition.dx - coords.relativeOffset.dx / devicePixelRatio) /
                coords.canvas.scale;
        double y2 =
            (localPosition.dy - coords.relativeOffset.dy / devicePixelRatio) /
                coords.canvas.scale;
        x2 += coords.cursor.offset.dx;
        y2 += coords.cursor.offset.dy;
        await parent.target?.inputModel.moveMouse(x2, y2);
        isMoved = true;
      }
    }
    if (!isMoved) {
      final rect = parent.target?.ffiModel.rect;
      if (rect == null) {
        // unreachable
        return;
      }

      Offset? movementInRect(double x, double y, Rect r) {
        final isXInRect = x >= r.left && x <= r.right;
        final isYInRect = y >= r.top && y <= r.bottom;
        if (!(isXInRect || isYInRect)) {
          return null;
        }
        if (x < r.left) {
          x = r.left;
        } else if (x > r.right) {
          x = r.right;
        }
        if (y < r.top) {
          y = r.top;
        } else if (y > r.bottom) {
          y = r.bottom;
        }
        return Offset(x, y);
      }

      final scale = parent.target?.canvasModel.scale ?? 1.0;
      var movement =
          movementInRect(_x + delta.dx / scale, _y + delta.dy / scale, rect);
      if (movement == null) {
        return;
      }
      movement = movementInRect(movement.dx, movement.dy, getVisibleRect());
      if (movement == null) {
        return;
      }

      _x = movement.dx;
      _y = movement.dy;
      await parent.target?.inputModel.moveMouse(_x, _y);
    }
    _notify();
  }

  disposeImages() {
    _images.forEach((_, v) => v.item1.dispose());
    _images.clear();
  }
}
