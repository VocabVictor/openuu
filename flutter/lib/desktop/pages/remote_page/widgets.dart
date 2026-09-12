part of 'remote_page.dart';

/// A widget that tracks the view size and updates CanvasModel.updateViewStyle()
/// and InputModel.updateImageWidgetSize() only when size actually changes.
/// This avoids scheduling post-frame callbacks on every LayoutBuilder rebuild.
class _ViewStyleUpdater extends StatefulWidget {
  final CanvasModel canvasModel;
  final InputModel inputModel;
  final Widget child;

  const _ViewStyleUpdater({
    Key? key,
    required this.canvasModel,
    required this.inputModel,
    required this.child,
  }) : super(key: key);

  @override
  State<_ViewStyleUpdater> createState() => _ViewStyleUpdaterState();
}

class _ViewStyleUpdaterState extends State<_ViewStyleUpdater> {
  Size? _lastSize;
  bool _callbackScheduled = false;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final maxWidth = constraints.maxWidth;
        final maxHeight = constraints.maxHeight;
        // Guard against infinite constraints (e.g., unconstrained ancestor).
        if (!maxWidth.isFinite || !maxHeight.isFinite) {
          return widget.child;
        }
        final newSize = Size(maxWidth, maxHeight);
        if (_lastSize != newSize) {
          _lastSize = newSize;
          // Schedule the update for after the current frame to avoid setState during build.
          // Use _callbackScheduled flag to prevent accumulating multiple callbacks
          // when size changes rapidly before any callback executes.
          if (!_callbackScheduled) {
            _callbackScheduled = true;
            SchedulerBinding.instance.addPostFrameCallback((_) {
              _callbackScheduled = false;
              final currentSize = _lastSize;
              if (mounted && currentSize != null) {
                widget.canvasModel.updateViewStyle();
                widget.inputModel.updateImageWidgetSize(currentSize);
              }
            });
          }
        }
        return widget.child;
      },
    );
  }
}

class CursorPaint extends StatelessWidget {
  final String id;
  final RxBool zoomCursor;

  const CursorPaint({
    Key? key,
    required this.id,
    required this.zoomCursor,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<CursorModel>(context);
    final c = Provider.of<CanvasModel>(context);
    double hotx = m.hotx;
    double hoty = m.hoty;
    if (m.image == null) {
      if (preDefaultCursor.image != null) {
        hotx = preDefaultCursor.image!.width / 2;
        hoty = preDefaultCursor.image!.height / 2;
      }
    }

    double cx = c.x;
    double cy = c.y;
    if (c.viewStyle.style == kRemoteViewStyleOriginal &&
        c.scrollStyle == ScrollStyle.scrollbar) {
      final rect = c.parent.target!.ffiModel.rect;
      if (rect == null) {
        // unreachable!
        debugPrint('unreachable! The displays rect is null.');
        return Container();
      }
      if (cx < 0) {
        final imageWidth = rect.width * c.scale;
        cx = -imageWidth * c.scrollX;
      }
      if (cy < 0) {
        final imageHeight = rect.height * c.scale;
        cy = -imageHeight * c.scrollY;
      }
    }

    double x = (m.x - hotx) * c.scale + cx;
    double y = (m.y - hoty) * c.scale + cy;
    double scale = 1.0;
    final isViewOriginal = c.viewStyle.style == kRemoteViewStyleOriginal;
    if (zoomCursor.value || isViewOriginal) {
      x = m.x - hotx + cx / c.scale;
      y = m.y - hoty + cy / c.scale;
      scale = c.scale;
    }

    return CustomPaint(
      painter: ImagePainter(
        image: m.image ?? preDefaultCursor.image,
        x: x,
        y: y,
        scale: scale,
      ),
    );
  }
}
