part of 'floating_mouse.dart';

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
