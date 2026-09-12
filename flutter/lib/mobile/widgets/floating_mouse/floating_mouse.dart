// This floating mouse widget simulates a physical mouse when connecting from mobile to desktop in touch mode.

import 'dart:async';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/image.dart';
import 'package:provider/provider.dart';
part 'mouse_build.dart';
part 'mouse_move.dart';
part 'floating_mouse_state.dart';
part 'canvas_scroll.dart';

const int _kDotCount = 60;
const double _kDotAngle = 2 * pi / _kDotCount;
final Color _kDefaultColor = Colors.grey.withOpacity(0.7);
final Color _kDefaultHighlightColor = Colors.white24.withOpacity(0.7);
final Color _kTapDownColor = Colors.blue.withOpacity(0.7);
const double _baseMouseWidth = 112.0;
const double _baseMouseHeight = 138.0;
const double _kShowPressedScale = 1.2;
const double kScaleMax = 1.8;
const double kScaleMin = 0.8;

double? _tryParseCoordinateFromEvt(Map<String, dynamic>? evt, String key) {
  if (evt == null) return null;
  final coord = evt[key];
  if (coord == null) return null;
  return double.tryParse(coord);
}

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
  bool _leftDown = false;
  bool _rightDown = false;
  bool _midDown = false;
  bool _dragDown = false;

  Widget _buildScrollUpDown(GlobalKey key, IconData iconData, double s) {
    return Container(
      key: key,
      height: 17 * s,
      child: Icon(
        iconData,
        color: _kDefaultHighlightColor,
        size: 14 * s,
      ),
    );
  }

  Widget _buildScrollMidButton(double s) {
    return Listener(
      onPointerDown: widget.inputModel != null
          ? (event) {
              widget.resetCollapseTimer?.call();
              setState(() {
                _midDown = true;
                widget.inputModel?.tapDown(MouseButtons.wheel);
              });
            }
          : null,
      onPointerUp: widget.inputModel != null
          ? (event) {
              setState(() {
                _midDown = false;
                widget.inputModel?.tapUp(MouseButtons.wheel);
                widget.cancelCanvasScroll?.call();
              });
            }
          : null,
      onPointerCancel: widget.inputModel != null
          ? (event) {
              setState(() {
                _midDown = false;
                widget.inputModel?.tapUp(MouseButtons.wheel);
                widget.cancelCanvasScroll?.call();
              });
            }
          : null,
      onPointerMove: widget.onPointerMoveUpdate,
      behavior: HitTestBehavior.opaque,
      child: Container(
        height: 28 * s,
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 6 * s,
                height: 2 * s,
                color: _kDefaultHighlightColor,
              ),
              SizedBox(height: 3 * s),
              Container(
                width: 8 * s,
                height: 2 * s,
                color: _kDefaultHighlightColor,
              ),
              SizedBox(height: 3 * s),
              Container(
                width: 6 * s,
                height: 2 * s,
                color: _kDefaultHighlightColor,
              ),
            ],
          ),
        ),
      ),
    );
  }

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

class DottedCirclePainter extends CustomPainter {
  final Offset center;
  final double pointerAngle;
  final double scale;
  final Offset? scrollWheelCenter;

  DottedCirclePainter(
      {required this.center,
      required this.pointerAngle,
      required this.scale,
      this.scrollWheelCenter});

  @override
  void paint(Canvas canvas, Size size) {
    final radius = 48.0 * scale;
    final circlePaint = Paint()
      ..color = Colors.grey.shade400
      ..style = PaintingStyle.fill;
    final pointerPaint = Paint()
      ..color = Colors.blue
      ..style = PaintingStyle.fill;

    const dotRadius = 2.5;
    for (int i = 0; i < _kDotCount; i += 3) {
      final angle = i * _kDotAngle;
      final dotX = center.dx + radius * cos(angle);
      final dotY = center.dy + radius * sin(angle);
      canvas.drawCircle(Offset(dotX, dotY), dotRadius, circlePaint);
    }

    final pointerX = center.dx + radius * cos(pointerAngle);
    final pointerY = center.dy + radius * sin(pointerAngle);
    final pointerPosition = Offset(pointerX, pointerY);
    canvas.drawCircle(pointerPosition, 8.0, pointerPaint);
  }

  @override
  bool shouldRepaint(covariant DottedCirclePainter oldDelegate) {
    return oldDelegate.pointerAngle != pointerAngle ||
        oldDelegate.center != center ||
        oldDelegate.scrollWheelCenter != scrollWheelCenter;
  }
}

// Painter for the bottom center indentation of the drag area
class BottomIndentPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.grey.withOpacity(0.7)
      ..style = PaintingStyle.fill;
    // Draw bottom semicircle
    final center = Offset(size.width / 2, size.height);
    canvas.drawArc(
      Rect.fromCenter(center: center, width: size.width, height: size.height),
      pi,
      pi,
      false,
      paint,
    );
    // Use background color to carve a circular notch in the middle
    final clearPaint = Paint()..blendMode = BlendMode.clear;
    canvas.drawCircle(Offset(size.width / 2, size.height - 10), 10, clearPaint);
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

// Painter for the top center indentation of the drag area
class DragAreaTopIndentPainter extends CustomPainter {
  final double scale;
  final Color color;
  DragAreaTopIndentPainter({required this.color, required this.scale});

  @override
  void paint(Canvas canvas, Size size) {
    // Use saveLayer to make the hollow part transparent
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.fill;
    canvas.saveLayer(Offset.zero & size, Paint());
    // Draw drag area main body (rectangle + bottom rounded corners)
    final rect = Rect.fromLTWH(0, 0, size.width, size.height);
    final rrect = RRect.fromRectAndCorners(
      rect,
      bottomLeft: Radius.circular(40 * scale),
      bottomRight: Radius.circular(40 * scale),
    );
    canvas.drawRRect(rrect, paint);
    // Use BlendMode.dstOut to carve a smaller semicircular notch at the top center
    final clearPaint = Paint()..blendMode = BlendMode.dstOut;
    canvas.drawArc(
      Rect.fromCenter(
          center: Offset(size.width / 2, 0),
          width: 25 * scale,
          height: 20 * scale),
      0,
      pi,
      false,
      clearPaint,
    );
    canvas.restore();
  }

  @override
  bool shouldRepaint(covariant DragAreaTopIndentPainter oldDelegate) {
    return oldDelegate.color != color || oldDelegate.scale != scale;
  }
}

class CursorPaint extends StatelessWidget {
  final double scale;
  CursorPaint({super.key, required this.scale});

  @override
  Widget build(BuildContext context) {
    final cursorModel = Provider.of<CursorModel>(context);
    double hotx = cursorModel.hotx;
    double hoty = cursorModel.hoty;
    var image = cursorModel.image;
    if (image == null) {
      if (preDefaultCursor.image != null) {
        image = preDefaultCursor.image;
        hotx = preDefaultCursor.image!.width / 2;
        hoty = preDefaultCursor.image!.height / 2;
      }
    }
    if (image == null) {
      return const Offstage();
    }
    assert(scale > 0, 'scale should always be positive');
    if (scale <= 0) {
      return const Offstage();
    }
    return CustomPaint(
      painter: ImagePainter(image: image, x: -hotx, y: -hoty, scale: scale),
    );
  }
}
