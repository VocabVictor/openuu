part of 'view_camera_page.dart';

class ImagePaint extends StatefulWidget {
  final FFI ffi;
  final String id;
  final RxBool cursorOverImage;
  final Widget Function(Widget)? listenerBuilder;

  ImagePaint(
      {Key? key,
      required this.ffi,
      required this.id,
      required this.cursorOverImage,
      this.listenerBuilder})
      : super(key: key);

  @override
  State<StatefulWidget> createState() => _ImagePaintState();
}

class _ImagePaintState extends State<ImagePaint> {
  String get id => widget.id;
  RxBool get cursorOverImage => widget.cursorOverImage;
  Widget Function(Widget)? get listenerBuilder => widget.listenerBuilder;

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    var c = Provider.of<CanvasModel>(context);
    final s = c.scale;

    bool isViewOriginal() => c.viewStyle.style == kRemoteViewStyleOriginal;

    if (c.imageOverflow.isTrue && c.scrollStyle != ScrollStyle.scrollauto) {
      final paintWidth = c.getDisplayWidth() * s;
      final paintHeight = c.getDisplayHeight() * s;
      final paintSize = Size(paintWidth, paintHeight);
      final paintWidget =
          m.useTextureRender || widget.ffi.ffiModel.pi.forceTextureRender
              ? _BuildPaintTextureRender(
                  c, s, Offset.zero, paintSize, isViewOriginal())
              : _buildScrollbarNonTextureRender(m, paintSize, s);
      return NotificationListener<ScrollNotification>(
        onNotification: (notification) {
          c.updateScrollPercent();
          return false;
        },
        child: Container(
            child: _buildCrossScrollbarFromLayout(
          context,
          _buildListener(paintWidget),
          c.size,
          paintSize,
          c.scrollHorizontal,
          c.scrollVertical,
        )),
      );
    } else {
      if (c.size.width > 0 && c.size.height > 0) {
        final paintWidget =
            m.useTextureRender || widget.ffi.ffiModel.pi.forceTextureRender
                ? _BuildPaintTextureRender(
                    c,
                    s,
                    Offset(
                      isLinux ? c.x.toInt().toDouble() : c.x,
                      isLinux ? c.y.toInt().toDouble() : c.y,
                    ),
                    c.size,
                    isViewOriginal())
                : _buildScrollAutoNonTextureRender(m, c, s);
        return Container(child: _buildListener(paintWidget));
      } else {
        return Container();
      }
    }
  }

  Widget _buildScrollbarNonTextureRender(
      ImageModel m, Size imageSize, double s) {
    return CustomPaint(
      size: imageSize,
      painter: ImagePainter(image: m.image, x: 0, y: 0, scale: s),
    );
  }

  Widget _buildScrollAutoNonTextureRender(
      ImageModel m, CanvasModel c, double s) {
    return CustomPaint(
      size: Size(c.size.width, c.size.height),
      painter: ImagePainter(image: m.image, x: c.x / s, y: c.y / s, scale: s),
    );
  }

  Widget _BuildPaintTextureRender(
      CanvasModel c, double s, Offset offset, Size size, bool isViewOriginal) {
    final ffiModel = c.parent.target!.ffiModel;
    final displays = ffiModel.pi.getCurDisplays();
    final children = <Widget>[];
    final rect = ffiModel.rect;
    if (rect == null) {
      return Container();
    }
    final curDisplay = ffiModel.pi.currentDisplay;
    for (var i = 0; i < displays.length; i++) {
      final textureId = widget.ffi.textureModel
          .getTextureId(curDisplay == kAllDisplayValue ? i : curDisplay);
      if (true) {
        // both "textureId.value != -1" and "true" seems ok
        children.add(Positioned(
          left: (displays[i].x - rect.left) * s + offset.dx,
          top: (displays[i].y - rect.top) * s + offset.dy,
          width: displays[i].width * s,
          height: displays[i].height * s,
          child: Obx(() => Texture(
                textureId: textureId.value,
                filterQuality:
                    isViewOriginal ? FilterQuality.none : FilterQuality.low,
              )),
        ));
      }
    }
    return SizedBox(
      width: size.width,
      height: size.height,
      child: Stack(children: children),
    );
  }

  MouseCursor _buildCustomCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = cursor.cache ?? preDefaultCursor.cache;
    return buildCursorOfCache(cursor, scale, cache);
  }

  MouseCursor _buildDisabledCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = preForbiddenCursor.cache;
    return buildCursorOfCache(cursor, scale, cache);
  }

  Widget _buildCrossScrollbarFromLayout(
    BuildContext context,
    Widget child,
    Size layoutSize,
    Size size,
    ScrollController horizontal,
    ScrollController vertical,
  ) {
    var widget = child;
    if (layoutSize.width < size.width) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: horizontal,
          scrollDirection: Axis.horizontal,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Row(
        children: [
          Container(
            width: ((layoutSize.width - size.width) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.height < size.height) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: vertical,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Column(
        children: [
          Container(
            height: ((layoutSize.height - size.height) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.width < size.width) {
      widget = RawScrollbar(
        thickness: kScrollbarThickness,
        thumbColor: Colors.grey,
        controller: horizontal,
        thumbVisibility: false,
        trackVisibility: false,
        notificationPredicate: layoutSize.height < size.height
            ? (notification) => notification.depth == 1
            : defaultScrollNotificationPredicate,
        child: widget,
      );
    }
    if (layoutSize.height < size.height) {
      widget = RawScrollbar(
        thickness: kScrollbarThickness,
        thumbColor: Colors.grey,
        controller: vertical,
        thumbVisibility: false,
        trackVisibility: false,
        child: widget,
      );
    }

    return Container(
      child: widget,
      width: layoutSize.width,
      height: layoutSize.height,
    );
  }

  Widget _buildListener(Widget child) {
    if (listenerBuilder != null) {
      return listenerBuilder!(child);
    } else {
      return child;
    }
  }
}
