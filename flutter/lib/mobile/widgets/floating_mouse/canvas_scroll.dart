part of 'floating_mouse.dart';

class _CanvasScrollState {
  static const double speedPressed = 3.0;
  final InputModel inputModel;
  final CanvasModel canvasModel;
  final int _intervalMillis = 30;
  Timer? _timer;
  double _dx = 0;
  double _dy = 0;
  double _speed = 1.0;
  Rect _displayRect = Rect.zero;
  Offset _mouseGlobalPosition = Offset.zero;

  _CanvasScrollState({required this.inputModel, required this.canvasModel});

  double get step => 5.0 * canvasModel.scale;

  set scrollX(double speed) {
    _dx = step;
    setSpeed(speed);
  }

  set scrollY(double speed) {
    _dy = step;
    setSpeed(speed);
  }

  void tryCancel() {
    _dx = 0;
    _dy = 0;
    if (_timer == null) return;
    _timer?.cancel();
    _timer = null;
  }

  void setPressedSpeed() {
    setSpeed(_speed > 0
        ? _CanvasScrollState.speedPressed
        : -_CanvasScrollState.speedPressed);
  }

  void setReleasedSpeed() {
    setSpeed(_speed > 0 ? 1.0 : -1.0);
  }

  void setSpeed(double newSpeed) {
    _speed = newSpeed;
    if (_speed > 0) {
      _speed = _speed.clamp(0.1, 10.0);
    } else {
      _speed = _speed.clamp(-10.0, -0.1);
    }
    if (_dx != 0) {
      _dx = step * _speed;
    } else if (_dy != 0) {
      _dy = step * _speed;
    }
  }

  void tryStart(Rect displayRect, Offset mouseGlobalPosition) {
    _displayRect = displayRect;
    _mouseGlobalPosition = mouseGlobalPosition;
    if (_timer != null) return;
    _timer = Timer.periodic(Duration(milliseconds: _intervalMillis), (timer) {
      if (_dx == 0 && _dy == 0) {
        tryCancel();
      } else {
        if (_dx != 0) {
          canvasModel.panX(_dx);
        }
        if (_dy != 0) {
          canvasModel.panY(_dy);
        }
        final evt = inputModel.processEventToPeer(
            InputModel.getMouseEventMove(), _mouseGlobalPosition,
            moveCanvas: false);
        if (shouldCancelScrollTimer(evt)) {
          tryCancel();
        }
      }
    });
  }

  bool shouldCancelScrollTimer(Map<String, dynamic>? evt) {
    if (evt == null) {
      return true;
    }
    double s = canvasModel.scale;
    assert(s > 0, 'canvasModel.scale should always be positive');
    if (s <= 0) {
      return true;
    }
    if (_dx != 0) {
      final x = _tryParseCoordinateFromEvt(evt, 'x');
      if (x == null) {
        return true;
      } else {
        if (_dx < 0) {
          if (isDoubleEqual(_displayRect.right - 1, x)) {
            return true;
          } else {
            final dxDisplay = _dx / s;
            if ((x - dxDisplay) > (_displayRect.right - 1)) {
              canvasModel.panX((x - _displayRect.right + 1) * s);
              return true;
            }
          }
        } else {
          if (isDoubleEqual(x, _displayRect.left)) {
            return true;
          } else {
            final dxDisplay = _dx / s;
            if ((x - dxDisplay) < _displayRect.left) {
              canvasModel.panX((x - _displayRect.left) * s);
              return true;
            }
          }
        }
      }
    }
    if (_dy != 0) {
      final y = _tryParseCoordinateFromEvt(evt, 'y');
      if (y == null) {
        return true;
      } else {
        if (_dy < 0) {
          if (isDoubleEqual(_displayRect.bottom - 1, y)) {
            return true;
          } else {
            final dyDisplay = _dy / s;
            if ((y - dyDisplay) > (_displayRect.bottom - 1)) {
              canvasModel.panY((y - _displayRect.bottom + 1) * s);
              return true;
            }
          }
        } else {
          if (isDoubleEqual(y, _displayRect.top)) {
            return true;
          } else {
            final dyDisplay = _dy / s;
            if ((y - dyDisplay) < _displayRect.top) {
              canvasModel.panY((y - _displayRect.top) * s);
              return true;
            }
          }
        }
      }
    }
    return false;
  }
}
