part of 'model.dart';

extension CanvasModelScroll on CanvasModel {
  void activateLocalCursor() {
    if (isDesktop || isWebDesktop) {
      try {
        RemoteCursorMovedState.find(id).value = false;
      } catch (e) {
        //
      }
    }
  }

  void updateLocalCursor(double x, double y) {
    if (parent.target?.ffiModel.viewOnly == true) return;
    // If keyboard is not permitted, do not move cursor when mouse is moving.
    if (parent.target != null && parent.target!.ffiModel.keyboard) {
      // Draw cursor if is not desktop.
      if (!(isDesktop || isWebDesktop)) {
        parent.target!.cursorModel.moveLocal(x, y);
      } else {
        try {
          RemoteCursorMovedState.find(id).value = false;
        } catch (e) {
          //
        }
      }
    }
  }

  void moveDesktopMouse(double x, double y) {
    if (size.width == 0 || size.height == 0) {
      return;
    }

    // On mobile platforms, move the canvas with the cursor.
    final dw = getDisplayWidth() * _scale;
    final dh = getDisplayHeight() * _scale;
    var dxOffset = 0;
    var dyOffset = 0;
    try {
      if (dw > size.width) {
        dxOffset = (x - dw * (x / size.width) - _x).toInt();
      }
      if (dh > size.height) {
        dyOffset = (y - dh * (y / size.height) - _y).toInt();
      }
    } catch (e) {
      debugPrintStack(
          label:
              '(x,y) ($x,$y), (_x,_y) ($_x,$_y), _scale $_scale, display size (${getDisplayWidth()},${getDisplayHeight()}), size $size, , $e');
      return;
    }

    _x += dxOffset;
    _y += dyOffset;
    if (dxOffset != 0 || dyOffset != 0) {
      _notify();
    }
  }

  void initializeEdgeScrollFallback(TickerProvider tickerProvider) {
    _edgeScrollFallbackState = EdgeScrollFallbackState(this, tickerProvider);
  }

  void disableEdgeScroll() {
    _edgeScrollState = EdgeScrollState.inactive;
    cancelEdgeScroll();
  }

  void rearmEdgeScroll() {
    _edgeScrollState = EdgeScrollState.armed;
  }

  void cancelEdgeScroll() {
    _edgeScrollFallbackState.stop();
  }

  (Vector2, Vector2) getScrollInfo() {
    final scrollPixel = Vector2(
        _horizontal.hasClients ? _horizontal.position.pixels : 0,
        _vertical.hasClients ? _vertical.position.pixels : 0);

    final max = Vector2(
        _horizontal.hasClients ? _horizontal.position.maxScrollExtent : 0,
        _vertical.hasClients ? _vertical.position.maxScrollExtent : 0);

    return (scrollPixel, max);
  }

  void edgeScrollMouse(double x, double y) async {
    if ((_edgeScrollState == EdgeScrollState.inactive) ||
        (size.width == 0 || size.height == 0) ||
        !(_horizontal.hasClients || _vertical.hasClients)) {
      return;
    }

    if (_edgeScrollState == EdgeScrollState.armed) {
      // Edge scroll is armed to become active once the cursor
      // is observed within the rectangle interior to the
      // edge scroll regions. If the user has just moved the
      // cursor in from outside of the window, edge scrolling
      // doesn't happen yet.
      final clientArea = Rect.fromLTWH(0, 0, size.width, size.height);

      final innerZone = clientArea.deflate(_edgeScrollEdgeThickness.toDouble());

      if (innerZone.contains(Offset(x, y))) {
        _edgeScrollState = EdgeScrollState.active;
      } else {
        // Not yet.
        return;
      }
    }

    var dxOffset = 0.0;
    var dyOffset = 0.0;

    if (x < _edgeScrollEdgeThickness) {
      dxOffset = x - _edgeScrollEdgeThickness;
    } else if (x >= size.width - _edgeScrollEdgeThickness) {
      dxOffset = x - (size.width - _edgeScrollEdgeThickness);
    }

    if (y < _edgeScrollEdgeThickness) {
      dyOffset = y - _edgeScrollEdgeThickness;
    } else if (y >= size.height - _edgeScrollEdgeThickness) {
      dyOffset = y - (size.height - _edgeScrollEdgeThickness);
    }

    var encroachment = Vector2(dxOffset, dyOffset);

    var (scrollPixel, max) = getScrollInfo();

    encroachment.clamp(-scrollPixel, max - scrollPixel);

    if (encroachment.length2 == 0) {
      _edgeScrollFallbackState.stop();
    } else {
      var bumpAmount = -encroachment;

      // Round away from 0: this ensures that the mouse will be bumped clear of
      // whichever edge scroll zone(s) it is in
      bumpAmount.x += bumpAmount.x.sign * 0.5;
      bumpAmount.y += bumpAmount.y.sign * 0.5;

      var bumpMouseSucceeded = _bumpMouseIsWorking &&
          (await rustDeskWinManager.call(WindowType.Main, kWindowBumpMouse,
                  {"dx": bumpAmount.x.round(), "dy": bumpAmount.y.round()}))
              .result;

      if (bumpMouseSucceeded) {
        performEdgeScroll(encroachment);
      } else {
        // If we can't BumpMouse, then we switch to slower scrolling with autorepeat

        // Don't keep hammering BumpMouse if it's not working.
        _bumpMouseIsWorking = false;

        // Keep scrolling as long as the user is overtop of an edge.
        _edgeScrollFallbackState.setEncroachment(encroachment);
        _edgeScrollFallbackState.start();
      }
    }
  }

  void performEdgeScroll(Vector2 delta) {
    var (scrollPixel, max) = getScrollInfo();

    scrollPixel += delta;

    scrollPixel.clamp(Vector2.zero(), max);

    var scrollPixelPercent = scrollPixel.clone();

    scrollPixelPercent.divide(max);
    scrollPixelPercent.scale(100.0);

    setScrollPercent(scrollPixelPercent.x, scrollPixelPercent.y);
    pushScrollPositionToUI(scrollPixel.x, scrollPixel.y);

    _notify();
  }
}
