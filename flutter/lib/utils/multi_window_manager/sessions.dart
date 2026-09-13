part of 'multi_window_manager.dart';

extension MultiWindowManagerSessions on RustDeskMultiWindowManager {
  // This function must be called in the main window thread.
  // Because the _remoteDesktopWindows is managed in that thread.
  openMonitorSession(int windowId, String peerId, int display, int displayCount,
      Rect? screenRect, int windowType) async {
    final isCamera = windowType == WindowType.ViewCamera.index;
    final windowIDs = isCamera ? _viewCameraWindows : _remoteDesktopWindows;
    if (windowIDs.length > 1) {
      for (final windowId in windowIDs) {
        if (await DesktopMultiWindow.invokeMethod(
            windowId,
            kWindowEventActiveDisplaySession,
            jsonEncode({
              'id': peerId,
              'display': display,
            }))) {
          return;
        }
      }
    }

    final displays = display == kAllDisplayValue
        ? List.generate(displayCount, (index) => index)
        : [display];
    var params = {
      'type': windowType,
      'id': peerId,
      'tab_window_id': windowId,
      'display': display,
      'displays': displays,
    };
    if (screenRect != null) {
      params['screen_rect'] = {
        'l': screenRect.left,
        't': screenRect.top,
        'r': screenRect.right,
        'b': screenRect.bottom,
      };
    }
    await _newSession(
      false,
      windowType.windowType,
      isCamera ? kWindowEventNewViewCamera : kWindowEventNewRemoteDesktop,
      peerId,
      windowIDs,
      jsonEncode(params),
      screenRect: screenRect,
    );
  }

  Future<int> newSessionWindow(
    WindowType type,
    String remoteId,
    String msg,
    List<int> windows,
    bool withScreenRect,
  ) async {
    final windowController = await DesktopMultiWindow.createWindow(msg);
    if (isWindows) {
      windowController.setInitBackgroundColor(Colors.black);
    }
    final windowId = windowController.windowId;
    if (!withScreenRect) {
      windowController
        ..setFrame(await _initialSessionFrame(windowId))
        ..center()
        ..setTitle(getWindowNameWithId(
          remoteId,
          overrideType: type,
        ));
    } else {
      windowController.setTitle(getWindowNameWithId(
        remoteId,
        overrideType: type,
      ));
    }
    if (isMacOS) {
      Future.microtask(() => windowController.show());
    }
    registerActiveWindow(windowId);
    windows.add(windowId);
    return windowId;
  }

  /// The frame a session window is created with, before the peer's display
  /// size is known: a 1280x720 (16:9) window shrunk to fit the work area,
  /// offset per window. The first peer info refits it (fitWindowToPeer).
  Future<Rect> _initialSessionFrame(int windowId) async {
    Rect work = const Rect.fromLTWH(0, 0, 1280, 720);
    try {
      final screen = await window_size.getCurrentScreen();
      if (screen != null) {
        work = logicalWorkArea(screen.visibleFrame, screen.scaleFactor);
      }
    } catch (_) {}
    final frame = fitRemoteWindowFrame(const Size(1280, 720), work);
    return frame.shift(Offset(windowId * 20.0, windowId * 20.0));
  }

  Future<MultiWindowCallResult> _newSession(
    bool openInTabs,
    WindowType type,
    String methodName,
    String remoteId,
    List<int> windows,
    String msg, {
    Rect? screenRect,
  }) async {
    if (openInTabs) {
      if (windows.isEmpty) {
        final windowId = await newSessionWindow(
            type, remoteId, msg, windows, screenRect != null);
        return MultiWindowCallResult(windowId, null);
      } else {
        return call(type, methodName, msg);
      }
    } else {
      if (_inactiveWindows.isNotEmpty) {
        for (final windowId in windows) {
          if (_inactiveWindows.contains(windowId)) {
            if (screenRect == null) {
              await restoreWindowPosition(type,
                  windowId: windowId, peerId: remoteId);
            }
            await DesktopMultiWindow.invokeMethod(windowId, methodName, msg);
            if (methodName != kWindowEventNewRemoteDesktop) {
              WindowController.fromWindowId(windowId).show();
            }
            registerActiveWindow(windowId);
            return MultiWindowCallResult(windowId, null);
          }
        }
      }
      final windowId = await newSessionWindow(
          type, remoteId, msg, windows, screenRect != null);
      return MultiWindowCallResult(windowId, null);
    }
  }
}
