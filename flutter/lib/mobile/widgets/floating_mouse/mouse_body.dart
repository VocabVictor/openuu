part of 'floating_mouse.dart';

class MouseBody extends StatefulWidget {
  final GlobalKey scrollWheelUpKey;
  final GlobalKey scrollWheelDownKey;
  final GlobalKey mouseWidgetKey;
  final Function(PointerMoveEvent)? onPointerMoveUpdate;
  final Function()? cancelCanvasScroll;
  final Function()? setCanvasScrollPressed;
  final Function()? setCanvasScrollReleased;
  final InputModel? inputModel;
  final double scale;
  final Function()? resetCollapseTimer;
  const MouseBody({
    super.key,
    required this.scrollWheelUpKey,
    required this.scrollWheelDownKey,
    required this.mouseWidgetKey,
    required this.scale,
    this.inputModel,
    this.onPointerMoveUpdate,
    this.cancelCanvasScroll,
    this.setCanvasScrollPressed,
    this.setCanvasScrollReleased,
    this.resetCollapseTimer,
  });

  @override
  State<MouseBody> createState() => _MouseBodyState();
}

class WidgetScale {
  final double scale;
  final double translateScale;

  const WidgetScale({required this.scale, required this.translateScale});

  static WidgetScale getScale(bool down, double s) {
    if (down) {
      return WidgetScale(
          scale: s * _kShowPressedScale,
          translateScale: s * (_kShowPressedScale - 1.0) * 0.5);
    } else {
      return WidgetScale(scale: s, translateScale: 0.0);
    }
  }
}

class _MouseBodyState extends State<MouseBody> {
  void _setState(VoidCallback fn) => setState(fn);
  bool _leftDown = false;
  bool _rightDown = false;
  bool _midDown = false;
  bool _dragDown = false;

  @override
  Widget build(BuildContext context) {
    final s = widget.scale;
    final leftScale = WidgetScale.getScale(_leftDown, s);
    final rightScale = WidgetScale.getScale(_rightDown, s);
    final midScale = WidgetScale.getScale(_midDown, s);
    return Row(
      children: [
        SizedBox(
          key: widget.mouseWidgetKey,
          width: 80 * s,
          height: 120 * s,
          child: Column(
            children: [
              SizedBox(
                height: 55 * s,
                child: Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        // Left button
                        Transform.translate(
                          offset: Offset(
                              -(80 - 24) * 0.5 * leftScale.translateScale,
                              -32 * leftScale.translateScale),
                          child: SizedBox(
                            width: (80 - 24) * 0.5 * leftScale.scale,
                            child: Listener(
                              onPointerMove: widget.onPointerMoveUpdate,
                              onPointerDown: widget.inputModel != null
                                  ? (event) {
                                      widget.resetCollapseTimer?.call();
                                      setState(() {
                                        _leftDown = true;
                                        widget.inputModel
                                            ?.tapDown(MouseButtons.left);
                                      });
                                    }
                                  : null,
                              onPointerUp: widget.inputModel != null
                                  ? (event) => setState(() {
                                        _leftDown = false;
                                        widget.inputModel
                                            ?.tapUp(MouseButtons.left);
                                        widget.cancelCanvasScroll?.call();
                                      })
                                  : null,
                              onPointerCancel: widget.inputModel != null
                                  ? (event) => setState(() {
                                        _leftDown = false;
                                        widget.inputModel
                                            ?.tapUp(MouseButtons.left);
                                        widget.cancelCanvasScroll?.call();
                                      })
                                  : null,
                              child: Container(
                                decoration: BoxDecoration(
                                  color: _leftDown
                                      ? _kTapDownColor
                                      : _kDefaultColor,
                                  borderRadius: BorderRadius.only(
                                      topLeft: Radius.circular(22 * s)),
                                ),
                                margin: EdgeInsets.only(right: 0.5 * s),
                              ),
                            ),
                          ),
                        ),
                        const Spacer(),
                        Transform.translate(
                          offset: Offset(
                              (80 - 24) * 0.5 * rightScale.translateScale,
                              -32 * rightScale.translateScale),
                          child: SizedBox(
                            width: (80 - 24) * 0.5 * rightScale.scale,
                            child: Listener(
                              onPointerMove: widget.onPointerMoveUpdate,
                              onPointerDown: widget.inputModel != null
                                  ? (event) {
                                      widget.resetCollapseTimer?.call();
                                      setState(() {
                                        _rightDown = true;
                                        widget.inputModel
                                            ?.tapDown(MouseButtons.right);
                                      });
                                    }
                                  : null,
                              onPointerUp: widget.inputModel != null
                                  ? (event) => setState(() {
                                        _rightDown = false;
                                        widget.inputModel
                                            ?.tapUp(MouseButtons.right);
                                        widget.cancelCanvasScroll?.call();
                                      })
                                  : null,
                              onPointerCancel: widget.inputModel != null
                                  ? (event) => setState(() {
                                        _rightDown = false;
                                        widget.inputModel
                                            ?.tapUp(MouseButtons.right);
                                        widget.cancelCanvasScroll?.call();
                                      })
                                  : null,
                              child: Container(
                                decoration: BoxDecoration(
                                  color: _rightDown
                                      ? _kTapDownColor
                                      : _kDefaultColor,
                                  borderRadius: BorderRadius.only(
                                      topRight: Radius.circular(22 * s)),
                                ),
                                margin: EdgeInsets.only(left: 0.5 * s),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                    // Middle function area overflows Row bottom
                    Positioned(
                      left: (80 * s - 22 * s) / 2,
                      top: 0,
                      child: Transform.translate(
                        offset: Offset(0, -2 * s),
                        child: Container(
                          width: 22 * s,
                          height: 67 * s,
                          decoration: BoxDecoration(
                            color: Colors.grey.withOpacity(0.7),
                            borderRadius: BorderRadius.vertical(
                              top: Radius.circular(12 * s),
                              bottom: Radius.circular(16 * s),
                            ),
                          ),
                          padding: EdgeInsets.symmetric(vertical: 2 * s),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                            children: [
                              _buildScrollUpDown(widget.scrollWheelUpKey,
                                  Icons.keyboard_arrow_up, midScale.scale),
                              _buildScrollMidButton(midScale.scale),
                              _buildScrollUpDown(widget.scrollWheelDownKey,
                                  Icons.keyboard_arrow_down, midScale.scale),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              // Thin gap separates upper and lower parts
              SizedBox(height: 1 * s),
              // Bottom part: drag area (top middle indentation)
              Expanded(
                child: Listener(
                  onPointerMove: widget.onPointerMoveUpdate,
                  onPointerDown: widget.inputModel != null
                      ? (event) {
                          widget.resetCollapseTimer?.call();
                          setState(() {
                            _dragDown = true;
                          });
                          widget.setCanvasScrollPressed?.call();
                        }
                      : null,
                  onPointerUp: widget.inputModel != null
                      ? (event) {
                          setState(() {
                            _dragDown = false;
                          });
                          widget.setCanvasScrollReleased?.call();
                        }
                      : null,
                  onPointerCancel: widget.inputModel != null
                      ? (event) {
                          setState(() {
                            _dragDown = false;
                          });
                          widget.setCanvasScrollReleased?.call();
                        }
                      : null,
                  behavior: HitTestBehavior.opaque,
                  child: CustomPaint(
                    painter: DragAreaTopIndentPainter(
                        color: _dragDown ? _kTapDownColor : _kDefaultColor,
                        scale: widget.scale),
                    child: Container(
                      width: 80 * s,
                      alignment: Alignment.center,
                      child: Transform.rotate(
                        angle: pi / 2,
                        child: Icon(Icons.drag_indicator,
                            color: _kDefaultHighlightColor, size: 18 * s),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
        const Spacer()
      ],
    );
  }
}
