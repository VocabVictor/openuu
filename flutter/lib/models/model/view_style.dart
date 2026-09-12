part of 'model.dart';

enum ScrollStyle {
  scrollbar(kRemoteScrollStyleBar),
  scrollauto(kRemoteScrollStyleAuto),
  scrolledge(kRemoteScrollStyleEdge);

  const ScrollStyle(this.stringValue);

  final String stringValue;

  String toJson() {
    return name;
  }

  static ScrollStyle fromJson(String json, [ScrollStyle? fallbackValue]) {
    switch (json) {
      case 'scrollbar':
        return scrollbar;
      case 'scrollauto':
        return scrollauto;
      case 'scrolledge':
        return scrolledge;
    }

    if (fallbackValue != null) {
      return fallbackValue;
    }

    throw ArgumentError("Unknown ScrollStyle JSON value: '$json'");
  }

  @override
  String toString() {
    return stringValue;
  }

  static ScrollStyle fromString(String string, [ScrollStyle? fallbackValue]) {
    switch (string) {
      case kRemoteScrollStyleBar:
        return scrollbar;
      case kRemoteScrollStyleAuto:
        return scrollauto;
      case kRemoteScrollStyleEdge:
        return scrolledge;
    }

    if (fallbackValue != null) {
      return fallbackValue;
    }

    throw ArgumentError("Unknown ScrollStyle string value: '$string'");
  }
}

class ViewStyle {
  final String style;
  final double width;
  final double height;
  final int displayWidth;
  final int displayHeight;
  ViewStyle({
    required this.style,
    required this.width,
    required this.height,
    required this.displayWidth,
    required this.displayHeight,
  });

  static defaultViewStyle() {
    final desktop = (isDesktop || isWebDesktop);
    final w =
        desktop ? kDesktopDefaultDisplayWidth : kMobileDefaultDisplayWidth;
    final h =
        desktop ? kDesktopDefaultDisplayHeight : kMobileDefaultDisplayHeight;
    return ViewStyle(
      style: '',
      width: w.toDouble(),
      height: h.toDouble(),
      displayWidth: w,
      displayHeight: h,
    );
  }

  static int _double2Int(double v) => (v * 100).round().toInt();

  @override
  bool operator ==(Object other) =>
      other is ViewStyle &&
      other.runtimeType == runtimeType &&
      _innerEqual(other);

  bool _innerEqual(ViewStyle other) {
    return style == other.style &&
        ViewStyle._double2Int(other.width) == ViewStyle._double2Int(width) &&
        ViewStyle._double2Int(other.height) == ViewStyle._double2Int(height) &&
        other.displayWidth == displayWidth &&
        other.displayHeight == displayHeight;
  }

  @override
  int get hashCode => Object.hash(
        style,
        ViewStyle._double2Int(width),
        ViewStyle._double2Int(height),
        displayWidth,
        displayHeight,
      ).hashCode;

  double get scale {
    double s = 1.0;
    if (style == kRemoteViewStyleAdaptive) {
      if (width != 0 &&
          height != 0 &&
          displayWidth != 0 &&
          displayHeight != 0) {
        final s1 = width / displayWidth;
        final s2 = height / displayHeight;
        s = s1 < s2 ? s1 : s2;
      }
    } else if (style == kRemoteViewStyleCustom) {
      // Custom scale is session-scoped and applied in CanvasModel.updateViewStyle()
    }
    return s;
  }
}

enum EdgeScrollState {
  inactive,
  armed,
  active,
}

class EdgeScrollFallbackState {
  final CanvasModel _owner;

  late Ticker _ticker;

  Duration _lastTotalElapsed = Duration.zero;
  bool _nextEventIsFirst = true;
  Vector2 _encroachment = Vector2.zero();

  EdgeScrollFallbackState(this._owner, TickerProvider tickerProvider) {
    _ticker = tickerProvider.createTicker(emitTick);
  }

  void setEncroachment(Vector2 encroachment) {
    _encroachment = encroachment;
  }

  void emitTick(Duration totalElapsed) {
    if (_nextEventIsFirst) {
      _lastTotalElapsed = totalElapsed;
      _nextEventIsFirst = false;
    } else {
      final thisTickElapsed = totalElapsed - _lastTotalElapsed;

      const double kFrameTime = 1000.0 / 60.0;
      const double kSpeedFactor = 0.1;

      var delta = _encroachment *
          (kSpeedFactor * thisTickElapsed.inMilliseconds / kFrameTime);

      _owner.performEdgeScroll(delta);

      _lastTotalElapsed = totalElapsed;
    }
  }

  void start() {
    if (!_ticker.isActive) {
      _nextEventIsFirst = true;
      _ticker.start();
    }
  }

  void stop() {
    _ticker.stop();
  }
}
