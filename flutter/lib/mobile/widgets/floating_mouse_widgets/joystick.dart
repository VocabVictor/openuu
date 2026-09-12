part of 'floating_mouse_widgets.dart';

// Virtual joystick can send either absolute movement (via updatePan)
// or relative movement (via sendMobileRelativeMouseMove) depending on the
// InputModel.relativeMouseMode setting.
class VirtualJoystick extends StatefulWidget {
  final CursorModel cursorModel;
  final InputModel inputModel;

  const VirtualJoystick({
    super.key,
    required this.cursorModel,
    required this.inputModel,
  });

  @override
  State<VirtualJoystick> createState() => _VirtualJoystickState();
}

class _VirtualJoystickState extends State<VirtualJoystick> {
  Offset _position = Offset.zero;
  bool _isInitialized = false;
  Offset _offset = Offset.zero;
  final double _joystickRadius = 50.0;
  final double _thumbRadius = 20.0;
  final double _moveStep = 3.0;
  final double _speed = 1.0;

  /// Scale factor for relative mouse movement sensitivity.
  /// Higher values result in faster cursor movement on the remote machine.
  static const double _kRelativeMouseScale = 3.0;

  // One-shot timer to detect a drag gesture
  Timer? _dragStartTimer;
  // Periodic timer for continuous movement
  Timer? _continuousMoveTimer;
  Size? _lastScreenSize;
  bool _isPressed = false;

  /// Check if relative mouse mode is enabled.
  bool get _useRelativeMouse => widget.inputModel.relativeMouseMode.value;

  @override
  void initState() {
    super.initState();
    widget.cursorModel.blockEvents = false;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _lastScreenSize = MediaQuery.of(context).size;
      _resetPosition();
    });
  }

  @override
  void dispose() {
    _stopSendEventTimer();
    widget.cursorModel.blockEvents = false;
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final currentScreenSize = MediaQuery.of(context).size;
    if (_lastScreenSize != null && _lastScreenSize != currentScreenSize) {
      _resetPosition();
    }
    _lastScreenSize = currentScreenSize;
  }

  void _resetPosition() {
    final size = MediaQuery.of(context).size;
    setState(() {
      _position = Offset(
        _kSpaceToHorizontalEdge + _joystickRadius,
        size.height * 0.5 + _joystickRadius * 1.5,
      );
      _isInitialized = true;
    });
  }

  Offset _offsetToPanDelta(Offset offset) {
    return Offset(
      offset.dx / _joystickRadius,
      offset.dy / _joystickRadius,
    );
  }

  /// Send movement delta to remote machine.
  /// Uses relative mouse mode if enabled, otherwise uses absolute updatePan.
  void _sendMovement(Offset delta) {
    if (_useRelativeMouse) {
      widget.inputModel.sendMobileRelativeMouseMove(
          delta.dx * _kRelativeMouseScale, delta.dy * _kRelativeMouseScale);
    } else {
      // In absolute mode, use cursorModel.updatePan which tracks position.
      widget.cursorModel.updatePan(delta, Offset.zero, false);
    }
  }

  void _stopSendEventTimer() {
    _dragStartTimer?.cancel();
    _continuousMoveTimer?.cancel();
    _dragStartTimer = null;
    _continuousMoveTimer = null;
  }

  @override
  Widget build(BuildContext context) {
    if (!_isInitialized) {
      return Positioned(child: Offstage());
    }
    return Positioned(
      left: _position.dx - _joystickRadius,
      top: _position.dy - _joystickRadius,
      child: GestureDetector(
        onPanStart: (details) {
          setState(() {
            _isPressed = true;
          });
          widget.cursorModel.blockEvents = true;
          _updateOffset(details.localPosition);

          // 1. Send a single, small pan event immediately for responsiveness.
          //    The movement is small for a gentle start.
          final initialDelta = _offsetToPanDelta(_offset);
          if (initialDelta.distance > 0) {
            _sendMovement(initialDelta);
          }

          // 2. Start a one-shot timer to check if the user is holding for a drag.
          _dragStartTimer?.cancel();
          _dragStartTimer = Timer(const Duration(milliseconds: 120), () {
            // 3. If the timer fires, it's a drag. Start the continuous movement timer.
            _continuousMoveTimer?.cancel();
            _continuousMoveTimer =
                periodic_immediate(const Duration(milliseconds: 20), () async {
              if (_offset != Offset.zero) {
                _sendMovement(_offsetToPanDelta(_offset) * _moveStep * _speed);
              }
            });
          });
        },
        onPanUpdate: (details) {
          _updateOffset(details.localPosition);
        },
        onPanEnd: (details) {
          setState(() {
            _offset = Offset.zero;
            _isPressed = false;
          });
          widget.cursorModel.blockEvents = false;

          // 4. Critical step: On pan end, cancel all timers.
          //    If it was a flick, this cancels the drag detection before it fires.
          //    If it was a drag, this stops the continuous movement.
          _stopSendEventTimer();
        },
        child: CustomPaint(
          size: Size(_joystickRadius * 2, _joystickRadius * 2),
          painter: _JoystickPainter(
              _offset, _joystickRadius, _thumbRadius, _isPressed),
        ),
      ),
    );
  }

  void _updateOffset(Offset localPosition) {
    final center = Offset(_joystickRadius, _joystickRadius);
    final offset = localPosition - center;
    final distance = offset.distance;

    if (distance <= _joystickRadius) {
      setState(() {
        _offset = offset;
      });
    } else {
      final clampedOffset = offset / distance * _joystickRadius;
      setState(() {
        _offset = clampedOffset;
      });
    }
  }
}

class _JoystickPainter extends CustomPainter {
  final Offset _offset;
  final double _joystickRadius;
  final double _thumbRadius;
  final bool _isPressed;

  _JoystickPainter(
      this._offset, this._joystickRadius, this._thumbRadius, this._isPressed);

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final joystickColor = _kDefaultColor;
    final borderColor = _isPressed ? _kTapDownColor : _kDefaultBorderColor;
    final thumbColor = _kWidgetHighlightColor;

    final joystickPaint = Paint()
      ..color = joystickColor
      ..style = PaintingStyle.fill;

    final borderPaint = Paint()
      ..color = borderColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;

    final thumbPaint = Paint()
      ..color = thumbColor
      ..style = PaintingStyle.fill;

    // Draw joystick base and border
    canvas.drawCircle(center, _joystickRadius, joystickPaint);
    canvas.drawCircle(center, _joystickRadius, borderPaint);

    // Draw thumb
    final thumbCenter = center + _offset;
    canvas.drawCircle(thumbCenter, _thumbRadius, thumbPaint);
  }

  @override
  bool shouldRepaint(covariant _JoystickPainter oldDelegate) {
    return oldDelegate._offset != _offset ||
        oldDelegate._isPressed != _isPressed;
  }
}

class _QuarterCirclePainter extends CustomPainter {
  final Color color;
  final bool isLeft;
  final double radius;
  _QuarterCirclePainter(
      {required this.color, required this.isLeft, required this.radius});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.fill;
    final rect = Rect.fromLTWH(0, 0, radius * 2, radius * 2);
    if (isLeft) {
      canvas.drawArc(rect, -pi, pi / 2, true, paint);
    } else {
      canvas.drawArc(rect, -pi / 2, pi / 2, true, paint);
    }
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) => false;
}
