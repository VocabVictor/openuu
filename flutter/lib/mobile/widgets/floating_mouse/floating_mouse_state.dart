part of 'floating_mouse.dart';

class FloatingMouse extends StatefulWidget {
  final FFI ffi;
  const FloatingMouse({
    super.key,
    required this.ffi,
  });

  @override
  State<FloatingMouse> createState() => _FloatingMouseState();
}

class _FloatingMouseState extends State<FloatingMouse> {
  void _setState(VoidCallback fn) => setState(fn);
  Rect? _lastBlockedRect;
  final GlobalKey _scrollWheelUpKey = GlobalKey();
  final GlobalKey _scrollWheelDownKey = GlobalKey();
  final GlobalKey _mouseWidgetKey = GlobalKey();
  final GlobalKey _cursorPaintKey = GlobalKey();

  Offset _position = Offset.zero;
  bool _isInitialized = false;
  double _baseMouseScale = 1.0;
  double _mouseScale = 1.0;
  bool _isExpanded = true;
  bool _isScrolling = false;
  Offset? _scrollCenter;
  double _snappedPointerAngle = 0.0;
  double? _lastSnappedAngle;
  late final _CanvasScrollState _canvasScrollState;
  Orientation? _previousOrientation;
  Timer? _collapseTimer;
  late final VirtualMouseMode _virtualMouseMode;

  void _resetCollapseTimer() {
    _collapseTimer?.cancel();
    if (_isExpanded) {
      _collapseTimer = Timer(const Duration(seconds: 7), () {
        if (mounted && _isExpanded) {
          final minMouseScale = (_baseMouseScale * 0.3);
          setState(() {
            _mouseScale = minMouseScale;
            _isExpanded = false;
            _position += _expandOffset;
          });
        }
      });
    }
  }

  double get mouseWidth => _baseMouseWidth * _mouseScale;
  double get mouseHeight => _baseMouseHeight * _mouseScale;

  InputModel get _inputModel => widget.ffi.inputModel;
  CursorModel get _cursorModel => widget.ffi.cursorModel;
  CanvasModel get _canvasModel => widget.ffi.canvasModel;

  Offset get _expandOffset =>
      Offset(84 * _baseMouseScale, 12 * _baseMouseScale);

  @override
  void initState() {
    super.initState();
    _virtualMouseMode = widget.ffi.ffiModel.virtualMouseMode;
    _virtualMouseMode.addListener(_onVirtualMouseModeChanged);
    _canvasScrollState =
        _CanvasScrollState(inputModel: _inputModel, canvasModel: _canvasModel);
    _cursorModel.blockEvents = false;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _resetPosition();
      _resetCollapseTimer();
    });
  }

  void _onVirtualMouseModeChanged() {
    if (mounted) {
      setState(() {
        if (_virtualMouseMode.showVirtualMouse) {
          _isExpanded = true;
          _resetCollapseTimer();
        }
      });
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final currentOrientation = MediaQuery.of(context).orientation;
    if (_previousOrientation != null &&
        _previousOrientation != currentOrientation) {
      _resetPosition();
    }
    _previousOrientation = currentOrientation;
  }

  void _resetPosition() {
    setState(() {
      final size = MediaQuery.of(context).size;
      _position = Offset(
        (size.width - _baseMouseWidth * _mouseScale) / 2,
        (size.height - _baseMouseHeight * _mouseScale) / 2,
      );
      _isInitialized = true;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _updateBlockedRect();
    });
  }

  @override
  void dispose() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    _virtualMouseMode.removeListener(_onVirtualMouseModeChanged);
    _canvasScrollState.tryCancel();
    _cursorModel.blockEvents = false;
    _collapseTimer?.cancel();
    super.dispose();
  }

  void _updateBlockedRect() {
    final context = _mouseWidgetKey.currentContext;
    if (context == null) return;
    final renderBox = context.findRenderObject() as RenderBox?;
    if (renderBox == null || !renderBox.attached) return;

    final newRect = renderBox.localToGlobal(Offset.zero) & renderBox.size;

    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    _cursorModel.addBlockedRect(newRect);
    _lastBlockedRect = newRect;
  }

  Offset _getMouseGlobalPosition() {
    final RenderBox? renderBox =
        _cursorPaintKey.currentContext?.findRenderObject() as RenderBox?;
    if (renderBox != null) {
      return renderBox.localToGlobal(Offset.zero);
    } else {
      return _position;
    }
  }

  static Offset? _getPositionFromMouseRetEvt(Map<String, dynamic>? evt) {
    final x = _tryParseCoordinateFromEvt(evt, 'x');
    final y = _tryParseCoordinateFromEvt(evt, 'y');
    if (x == null || y == null) {
      return null;
    }
    return Offset(x, y);
  }

  // Returns true if [value] is within 2.01 pixels of [edge].
  // We need this near check because it can make the auto scroll easier to trigger and control.
  bool _isValueNearEdge(double edge, double value) {
    return (value - edge).abs() < 2.01;
  }

  bool _isValueAtEdge(double edge, double value) {
    return (value - edge).abs() < 0.01;
  }

  bool _isValueAtOrOutsideEdge(double edge, double? value) {
    // If value is null, then consider it outside the edge.
    return value == null || isDoubleEqual(value, edge);
  }

  void _handlePointerUp(PointerUpEvent event) => _tryCancelScrolling();
  void _handlePointerCancel(PointerCancelEvent event) => _tryCancelScrolling();

  @override
  Widget build(BuildContext context) {
    if (!_isInitialized) {
      return const Offstage();
    }
    final virtualMouseMode = _virtualMouseMode;
    if (!virtualMouseMode.showVirtualMouse) {
      return const Offstage();
    }
    _baseMouseScale = virtualMouseMode.virtualMouseScale;
    if (_isExpanded) {
      _mouseScale = _baseMouseScale;
    } else {
      final minMouseScale = (_baseMouseScale * 0.3);
      _mouseScale = minMouseScale;
    }
    return Listener(
      onPointerDown: _isExpanded ? _handlePointerDown : null,
      onPointerMove: _handlePointerMove,
      onPointerUp: _handlePointerUp,
      onPointerCancel: _handlePointerCancel,
      behavior: HitTestBehavior.translucent,
      child: Stack(
        children: [
          if (!_isScrolling)
            Positioned(
              left: _position.dx,
              top: _position.dy,
              child: _buildMouseWithHide(),
            ),
          if (_isScrolling && _scrollCenter != null)
            Positioned.fill(
              child: Builder(
                builder: (context) {
                  final RenderBox? customPaintBox =
                      context.findRenderObject() as RenderBox?;
                  if (customPaintBox == null || !customPaintBox.attached) {
                    WidgetsBinding.instance.addPostFrameCallback((_) {
                      if (mounted && _isScrolling) setState(() {});
                    });
                    return const SizedBox.expand();
                  }
                  final Offset customPaintTopLeft =
                      customPaintBox.localToGlobal(Offset.zero);
                  final Offset localCenter =
                      _scrollCenter! - customPaintTopLeft;
                  return CustomPaint(
                    painter: DottedCirclePainter(
                      center: localCenter,
                      pointerAngle: _snappedPointerAngle,
                      scale: _mouseScale,
                    ),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }

}
