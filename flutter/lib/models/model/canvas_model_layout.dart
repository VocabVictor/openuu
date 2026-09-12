part of 'model.dart';
// ignore_for_file: invalid_use_of_protected_member, invalid_use_of_visible_for_testing_member

extension CanvasModelLayout on CanvasModel {
  Size getSize() {
    final mediaData = MediaQueryData.fromView(ui.window);
    final size = mediaData.size;
    // If minimized, w or h may be negative here.
    double w = size.width - CanvasModel.leftToEdge - CanvasModel.rightToEdge;
    double h = size.height - CanvasModel.topToEdge - CanvasModel.bottomToEdge;
    if (isMobile) {
      // Account for horizontal safe area insets on both orientations.
      w = w - mediaData.padding.left - mediaData.padding.right;
      // Vertically, subtract the bottom keyboard inset (viewInsets.bottom) and any
      // bottom overlay (e.g. key-help tools) so the canvas is not covered.
      h = h -
          mediaData.viewInsets.bottom -
          (parent.target?.cursorModel.keyHelpToolsRectToAdjustCanvas?.bottom ??
              0);
      // Orientation-specific handling:
      //  - Portrait: additionally subtract top padding (e.g. status bar / notch)
      //  - Landscape: does not subtract mediaData.padding.top/bottom (home indicator auto-hides)
      final isPortrait = size.height > size.width;
      if (isPortrait) {
        // In portrait mode, subtract the top safe-area padding (e.g. status bar / notch)
        // so the remote image is not truncated, while keeping the bottom inset to avoid
        // introducing unnecessary blank space around the canvas.
        //
        // iOS -> Android, portrait, adjust mode:
        // h = h (no padding subtracted): top and bottom are truncated
        //   https://github.com/user-attachments/assets/30ed4559-c27e-432b-847f-8fec23c9f998
        // h = h - top - bottom: extra blank spaces appear
        //   https://github.com/user-attachments/assets/12a98817-3b4e-43aa-be0f-4b03cf364b7e
        // h = h - top (current): works fine
        //   https://github.com/user-attachments/assets/95f047f2-7f47-4a36-8113-5023989a0c81
        h = h - mediaData.padding.top;
      }
    }
    return Size(w < 0 ? 0 : w, h < 0 ? 0 : h);
  }

  // mobile only
  double getAdjustY() {
    final bottom =
        parent.target?.cursorModel.keyHelpToolsRectToAdjustCanvas?.bottom ?? 0;
    return max(bottom - MediaQueryData.fromView(ui.window).padding.top, 0);
  }

  updateSize() => _size = getSize();

  updateViewStyle({refreshMousePos = true, notify = true}) async {
    final style = await bind.sessionGetViewStyle(sessionId: sessionId);
    if (style == null) {
      return;
    }

    updateSize();
    final displayWidth = getDisplayWidth();
    final displayHeight = getDisplayHeight();
    final viewStyle = ViewStyle(
      style: style,
      width: size.width,
      height: size.height,
      displayWidth: displayWidth,
      displayHeight: displayHeight,
    );
    // If only the Custom scale percent changed, proceed to update even if
    // the basic ViewStyle fields are equal.
    // In Custom scale mode, the scale percent can change independently of the other
    // ViewStyle fields and is not captured by the equality check. Therefore, we must
    // allow updates to proceed when style == kRemoteViewStyleCustom, even if the
    // rest of the ViewStyle fields are unchanged.
    if (_lastViewStyle == viewStyle && style != kRemoteViewStyleCustom) {
      return;
    }
    if (_lastViewStyle.style != viewStyle.style) {
      _resetScroll();
    }
    _lastViewStyle = viewStyle;
    _scale = viewStyle.scale;

    // Apply custom scale percent when in Custom mode
    if (style == kRemoteViewStyleCustom) {
      try {
        _scale = await getSessionCustomScale(sessionId);
      } catch (e, stack) {
        debugPrint('Error in getSessionCustomScale: $e');
        debugPrintStack(stackTrace: stack);
        _scale = 1.0;
      }
    }

    _devicePixelRatio = ui.window.devicePixelRatio;
    if (kIgnoreDpi) {
      if (style == kRemoteViewStyleOriginal) {
        _scale = 1.0 / _devicePixelRatio;
      } else if (_scale != 0 && style == kRemoteViewStyleCustom) {
        _scale /= _devicePixelRatio;
      }
    }
    _resetCanvasOffset(displayWidth, displayHeight);
    final overflow = _x < 0 || y < 0;
    if (_imageOverflow.value != overflow) {
      _imageOverflow.value = overflow;
    }
    if (notify) {
      notifyListeners();
    }
    if (!isMobile && refreshMousePos) {
      parent.target?.inputModel.refreshMousePos();
    }
    tryUpdateScrollStyle(Duration.zero, style);
  }

  _resetCanvasOffset(int displayWidth, int displayHeight) {
    _x = (size.width - displayWidth * _scale) / 2;
    _y = (size.height - displayHeight * _scale) / 2;
    if (isMobile) {
      _moveToCenterCursor();
    }
  }

  tryUpdateScrollStyle(Duration duration, String? style) async {
    if (_scrollStyle == ScrollStyle.scrollauto) return;
    style ??= await bind.sessionGetViewStyle(sessionId: sessionId);
    if (style != kRemoteViewStyleOriginal && style != kRemoteViewStyleCustom) {
      return;
    }

    _resetScroll();

    Future.delayed(duration, () async {
      updateScrollPercent();
    });
  }

  Future<void> updateScrollStyle() async {
    final style = await bind.sessionGetScrollStyle(sessionId: sessionId);

    _scrollStyle =
        style != null ? ScrollStyle.fromString(style) : ScrollStyle.scrollauto;

    if (_scrollStyle != ScrollStyle.scrollauto) {
      _resetScroll();
    }

    notifyListeners();
  }

  Future<void> initializeEdgeScrollEdgeThickness() async {
    final savedValue =
        await bind.sessionGetEdgeScrollEdgeThickness(sessionId: sessionId);

    if (savedValue != null) {
      _edgeScrollEdgeThickness = savedValue;
    }
  }

  void updateEdgeScrollEdgeThickness(int newThickness) {
    _edgeScrollEdgeThickness = newThickness;
    notifyListeners();
  }

  void update(double x, double y, double scale) {
    _x = x;
    _y = y;
    _scale = scale;
    notifyListeners();
  }
}
