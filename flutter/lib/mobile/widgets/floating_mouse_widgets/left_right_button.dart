part of 'floating_mouse_widgets.dart';

class FloatingLeftRightButton extends StatefulWidget {
  final bool isLeft;
  final InputModel inputModel;
  final CursorModel cursorModel;
  const FloatingLeftRightButton(
      {super.key,
      required this.isLeft,
      required this.inputModel,
      required this.cursorModel});

  @override
  State<FloatingLeftRightButton> createState() =>
      _FloatingLeftRightButtonState();
}

class _FloatingLeftRightButtonState extends State<FloatingLeftRightButton> {
  void _setState(VoidCallback fn) => setState(fn);
  Offset _position = Offset.zero;
  bool _isInitialized = false;
  bool _isDown = false;
  Rect? _lastBlockedRect;

  Orientation? _previousOrientation;
  Offset _preSavedPos = Offset.zero;

  // Gesture ambiguity resolution
  Timer? _tapDownTimer;
  final Duration _pressTimeout = const Duration(milliseconds: 200);
  bool _isDragging = false;

  bool get _isLeft => widget.isLeft;
  InputModel get _inputModel => widget.inputModel;
  CursorModel get _cursorModel => widget.cursorModel;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final currentOrientation = MediaQuery.of(context).orientation;
      _previousOrientation = currentOrientation;
      _resetPosition(currentOrientation);
    });
  }

  @override
  void dispose() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    _tapDownTimer?.cancel();
    _trySavePosition();
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final currentOrientation = MediaQuery.of(context).orientation;
    if (_previousOrientation == null ||
        _previousOrientation != currentOrientation) {
      _resetPosition(currentOrientation);
    }
    _previousOrientation = currentOrientation;
  }

  double _getOffsetX(double w) {
    if (_isLeft) {
      return (w - _kLeftRightButtonWidth * 2 - _kSpaceBetweenLeftRightButtons) *
          0.5;
    } else {
      return (w + _kSpaceBetweenLeftRightButtons) * 0.5;
    }
  }

  String _getPositionKey(Orientation ori) {
    final strLeftRight = _isLeft ? 'l' : 'r';
    final strOri = ori == Orientation.landscape ? 'l' : 'p';
    return '$strLeftRight$strOri-mouse-btn-pos';
  }

  static Offset? _loadPositionFromString(String s) {
    if (s.isEmpty) {
      return null;
    }
    try {
      final m = jsonDecode(s);
      return Offset(m['x'], m['y']);
    } catch (e) {
      debugPrintStack(label: 'Failed to load position "$s" $e');
      return null;
    }
  }

  Widget _buildButtonIcon() {
    final double w = _kLeftRightButtonWidth * 0.45;
    final double h = _kLeftRightButtonHeight * 0.75;
    final double borderRadius = w * 0.5;
    final double quarterCircleRadius = borderRadius * 0.9;
    return Stack(
      children: [
        Container(
          width: w,
          height: h,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(_kLeftRightButtonWidth * 0.225),
            color: Colors.white,
          ),
        ),
        Positioned(
          left: _isLeft ? quarterCircleRadius * 0.25 : null,
          right: _isLeft ? null : quarterCircleRadius * 0.25,
          top: quarterCircleRadius * 0.25,
          child: CustomPaint(
            size: Size(quarterCircleRadius * 2, quarterCircleRadius * 2),
            painter: _QuarterCirclePainter(
              color: _kDefaultColor,
              isLeft: _isLeft,
              radius: quarterCircleRadius,
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (!_isInitialized) {
      return Positioned(child: Offstage());
    }
    return Positioned(
      left: _position.dx,
      top: _position.dy,
      // We can't use the GestureDetector here, because `onTapDown` may be
      // triggered sometimes when dragging.
      child: Listener(
        onPointerMove: _onBodyPointerMoveUpdate,
        onPointerDown: (event) async {
          _isDragging = false;
          setState(() {
            _isDown = true;
          });
          // Start a timer. If it fires, it's a hold.
          _tapDownTimer?.cancel();
          _tapDownTimer = Timer(_pressTimeout, () {
            isSpecialHoldDragActive = true;
            () async {
              await _cursorModel.syncCursorPosition();
              await _inputModel
                  .tapDown(_isLeft ? MouseButtons.left : MouseButtons.right);
            }();
            _tapDownTimer = null;
          });
        },
        onPointerUp: (event) {
          _cursorModel.blockEvents = false;
          setState(() {
            _isDown = false;
          });
          // If timer is active, it's a quick tap.
          if (_tapDownTimer != null) {
            _tapDownTimer!.cancel();
            _tapDownTimer = null;
            // Fire tap down and up quickly.
            _inputModel
                .tapDown(_isLeft ? MouseButtons.left : MouseButtons.right)
                .then(
                    (_) => Future.delayed(const Duration(milliseconds: 50), () {
                          _inputModel.tapUp(
                              _isLeft ? MouseButtons.left : MouseButtons.right);
                        }));
          } else {
            // If it's not a quick tap, it could be a hold or drag.
            // If it was a hold, isSpecialHoldDragActive is true.
            if (isSpecialHoldDragActive) {
              _inputModel
                  .tapUp(_isLeft ? MouseButtons.left : MouseButtons.right);
            }
          }

          if (_isDragging) {
            _trySavePosition();
          }
          isSpecialHoldDragActive = false;
        },
        onPointerCancel: (event) {
          _cursorModel.blockEvents = false;
          setState(() {
            _isDown = false;
          });
          _tapDownTimer?.cancel();
          _tapDownTimer = null;
          if (isSpecialHoldDragActive) {
            _inputModel.tapUp(_isLeft ? MouseButtons.left : MouseButtons.right);
          }
          isSpecialHoldDragActive = false;
          if (_isDragging) {
            _trySavePosition();
          }
        },
        child: Container(
          width: _kLeftRightButtonWidth,
          height: _kLeftRightButtonHeight,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: _kDefaultColor,
            border: Border.all(
                color: _isDown ? _kTapDownColor : _kDefaultBorderColor,
                width: _kBorderWidth),
            borderRadius: _isLeft
                ? BorderRadius.horizontal(
                    left: Radius.circular(_kLeftRightButtonHeight * 0.5))
                : BorderRadius.horizontal(
                    right: Radius.circular(_kLeftRightButtonHeight * 0.5)),
          ),
          child: _buildButtonIcon(),
        ),
      ),
    );
  }
}
