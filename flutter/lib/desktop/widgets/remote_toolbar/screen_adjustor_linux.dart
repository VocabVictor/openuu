part of 'remote_toolbar.dart';

extension _ScreenAdjustorLinux on ScreenAdjustor {
  Future<void> _updateLinuxWorkAreaCache({
    required window_size.Screen screen,
    required Rect wndRect,
    required bool isWayland,
    required bool isX11,
    required bool forMenu,
  }) async {
    if (isWayland &&
        (_waylandWorkAreaScreenFrame != screen.frame ||
            _waylandWorkAreaScaleFactor != screen.scaleFactor)) {
      _waylandMaximizedWorkAreaSize = null;
      _waylandWorkAreaScreenFrame = screen.frame;
      _waylandWorkAreaScaleFactor = screen.scaleFactor;
    }
    if (isWayland &&
        forMenu &&
        !isFullscreen &&
        await isWindowMaximized() == true) {
      _waylandMaximizedWorkAreaSize = wndRect.size;
    }
    if (isX11 &&
        (_x11WorkAreaScreenFrame != screen.frame ||
            _x11WorkAreaScaleFactor != screen.scaleFactor)) {
      _x11WorkArea = null;
      _x11WorkAreaScreenFrame = screen.frame;
      _x11WorkAreaScaleFactor = screen.scaleFactor;
    }
    if (isX11 && forMenu && !isFullscreen) {
      _x11WorkArea = screen.visibleFrame;
    }
  }

  Future<Rect?> _getEffectiveScreenFrame({
    required window_size.Screen screen,
    required bool isWayland,
    required bool isX11,
    required bool forMenu,
  }) async {
    Rect frameRect = screen.visibleFrame;
    if (isMacOS && forMenu && isFullscreen) {
      List<double>? workArea;
      try {
        workArea = await kMacOSPermChannel
            .invokeListMethod<double>('getMacOSWorkAreaSize');
      } catch (_) {
        return null;
      }
      if (workArea == null || workArea.length != 2) {
        return null;
      }
      frameRect = Rect.fromLTWH(
        frameRect.left,
        frameRect.top,
        workArea[0] < frameRect.width ? workArea[0] : frameRect.width,
        workArea[1] < frameRect.height ? workArea[1] : frameRect.height,
      );
    }
    final x11WorkArea = _x11WorkArea;
    if (isX11 &&
        forMenu &&
        isFullscreen &&
        x11WorkArea != null &&
        (x11WorkArea.width < frameRect.width ||
            x11WorkArea.height < frameRect.height)) {
      frameRect = x11WorkArea;
    }
    final screenScale = screen.scaleFactor;
    if (isWayland && screenScale > 1.01) {
      String monitorLayoutMode;
      try {
        monitorLayoutMode =
            await bind.mainGetCommon(key: 'gnome-monitor-layout-mode');
      } catch (_) {
        monitorLayoutMode = '';
      }
      if (monitorLayoutMode == 'physical') {
        frameRect = Rect.fromLTRB(
          frameRect.left / screenScale,
          frameRect.top / screenScale,
          frameRect.right / screenScale,
          frameRect.bottom / screenScale,
        );
      }
    }
    return frameRect;
  }

  Future<Rect?> _getAdjustedWindowFrame(Size mediaSize,
      {bool forMenu = false}) async {
    final screen = _screen;
    if (screen != null) {
      // Windows window frames use physical pixels while Flutter view sizes are
      // logical. macOS and Linux window frames use the same units as Flutter.
      double scale = isWindows ? screen.scaleFactor : 1.0;
      final Rect wndRect;
      try {
        wndRect = await WindowController.fromWindowId(windowId).getFrame();
      } catch (e) {
        debugPrint("Failed to get frame of window $windowId, it may be hidden");
        return null;
      }
      // On Windows, wndRect is GetWindowRect while mediaSize is GetClientRect.
      // https://stackoverflow.com/a/7561083
      double magicWidth =
          wndRect.right - wndRect.left - mediaSize.width * scale;
      double magicHeight =
          wndRect.bottom - wndRect.top - mediaSize.height * scale;
      final canvasModel = this.ffi.canvasModel;
      // canvasModel.scale is the rendered scale and already applies kIgnoreDpi.
      // Use it instead of the remote source resolution.
      final isWayland = isLinux && bind.mainCurrentIsWayland();
      final isX11 = isLinux && !isWayland;
      await _updateLinuxWorkAreaCache(
        screen: screen,
        wndRect: wndRect,
        isWayland: isWayland,
        isX11: isX11,
        forMenu: forMenu,
      );
      if (isWindows && forMenu && isFullscreen) {
        // desktop_multi_window's hidden title bar keeps 8 physical pixels on
        // each horizontal edge and at the bottom, plus up to 1px at the top.
        // Fullscreen removes these in WM_NCCALCSIZE, so predict the restored
        // frame's worst-case padding when deciding whether to show the menu.
        magicWidth = 16.0;
        magicHeight = 9.0;
      }
      double horizontalEdges;
      double verticalEdges;
      if (forMenu && (isLinux || ((isMacOS || isWindows) && isFullscreen))) {
        // Linux Adjust Window unmaximizes; macOS and Windows exit fullscreen
        // before resizing. Predict the restored normal-window edges when
        // deciding whether to show the menu item.
        final resizePadding = isLinux && !kUseCompatibleUiMode
            ? kDragToResizeAreaPaddingSize
            : 0.0;
        final windowEdge = kWindowBorderWidth + resizePadding;
        horizontalEdges = windowEdge * 2;
        verticalEdges = kDesktopRemoteTabBarHeight + windowEdge * 2;
      } else {
        horizontalEdges = CanvasModel.leftToEdge + CanvasModel.rightToEdge;
        verticalEdges = CanvasModel.topToEdge + CanvasModel.bottomToEdge;
      }
      final width = (canvasModel.getDisplayWidth() * canvasModel.scale +
                  horizontalEdges) *
              scale +
          magicWidth;
      final height =
          (canvasModel.getDisplayHeight() * canvasModel.scale + verticalEdges) *
                  scale +
              magicHeight;
      double left = wndRect.left + (wndRect.width - width) / 2;
      double top = wndRect.top + (wndRect.height - height) / 2;

      final frameRect = await _getEffectiveScreenFrame(
        screen: screen,
        isWayland: isWayland,
        isX11: isX11,
        forMenu: forMenu,
      );
      if (frameRect == null) {
        return null;
      }
      var availableSize = frameRect.size;
      if (isWayland && forMenu && _waylandMaximizedWorkAreaSize != null) {
        final cachedSize = _waylandMaximizedWorkAreaSize!;
        availableSize = Size(
          cachedSize.width < availableSize.width
              ? cachedSize.width
              : availableSize.width,
          cachedSize.height < availableSize.height
              ? cachedSize.height
              : availableSize.height,
        );
      }
      // A window frame cannot be smaller than its client area. Tolerate small
      // floating-point differences; larger negative values mean the native
      // frame and Flutter view metrics are not synchronized.
      if (magicWidth < -0.1 || magicHeight < -0.1) {
        return null;
      }
      // Reject implausibly small targets to avoid hiding the window.
      if (width < 300 || height < 300) {
        return null;
      }
      // The remote size may change after the menu is built. Reject targets
      // that exceed the available area.
      final exceedsScreen =
          width > availableSize.width || height > availableSize.height;
      if (exceedsScreen) {
        return null;
      }
      if (left < frameRect.left) {
        left = frameRect.left;
      }
      if (top < frameRect.top) {
        top = frameRect.top;
      }
      if ((left + width) > frameRect.right) {
        left = frameRect.right - width;
      }
      if ((top + height) > frameRect.bottom) {
        top = frameRect.bottom - height;
      }
      return Rect.fromLTWH(left, top, width, height);
    }
    return null;
  }

}
