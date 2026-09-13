part of 'remote_toolbar.dart';

class ScreenAdjustor {
  final String id;
  final FFI ffi;
  final VoidCallback cbExitFullscreen;
  window_size.Screen? _screen;
  Size? _waylandMaximizedWorkAreaSize;
  Rect? _waylandWorkAreaScreenFrame;
  double? _waylandWorkAreaScaleFactor;
  Rect? _x11WorkArea;
  Rect? _x11WorkAreaScreenFrame;
  double? _x11WorkAreaScaleFactor;

  ScreenAdjustor({
    required this.id,
    required this.ffi,
    required this.cbExitFullscreen,
  });

  bool get isFullscreen => stateGlobal.fullscreen.isTrue;
  int get windowId => stateGlobal.windowId;

  Future<bool?> isWindowMaximized() async {
    try {
      return await WindowController.fromWindowId(windowId).isMaximized();
    } catch (_) {
      // The delayed resolution callback may run after the window is disposed.
      return null;
    }
  }

  adjustWindow(BuildContext context) {
    return futureBuilder(
        future: isWindowCanBeAdjusted(context),
        hasData: (data) {
          final visible = data as bool;
          if (!visible) return Offstage();
          return Column(
            children: [
              MenuButton(
                  child: Text(translate('Adjust Window')),
                  onPressed: () => doAdjustWindow(context),
                  ffi: ffi),
              Divider(),
            ],
          );
        });
  }

  // Linux screen and work-area coordinates can use different units or become
  // unreliable across Wayland/X11 state changes, so normalize reported frames
  // and cache usable work-area measurements before sizing the window.

  doAdjustWindow([BuildContext? context]) async {
    // A resolution change is adjusted after a delay, when the menu context may
    // already be disposed. Each desktop_multi_window window has its own engine,
    // so that engine's first view is the current window.
    final views = WidgetsBinding.instance.platformDispatcher.views;
    if (context == null && views.isEmpty) {
      return;
    }
    final view = context != null ? View.of(context) : views.first;
    await updateScreen();
    if (_screen != null) {
      final wc = WindowController.fromWindowId(windowId);
      final wasFullscreen = isFullscreen;
      cbExitFullscreen();
      if (wasFullscreen) {
        // Wait for the native fullscreen exit to update the window frame.
        await Future.delayed(Duration(milliseconds: 700));
        await updateScreen();
      }
      if (isLinux) {
        final isMaximized = await isWindowMaximized();
        if (isMaximized == null) {
          return;
        }
        if (isMaximized == true) {
          // setFrame may be ignored while the native window is maximized.
          try {
            await wc.unmaximize();
          } catch (_) {
            return;
          }
          stateGlobal.setMaximized(false);
          // Wait for the window manager and Flutter view metrics to reflect
          // the restored window before calculating and setting its frame.
          await Future.delayed(Duration(milliseconds: 300));
          await updateScreen();
        }
      }
      final mediaSize = MediaQueryData.fromView(view).size;
      final frame = await _getAdjustedWindowFrame(mediaSize);
      if (frame == null) {
        return;
      }
      try {
        await wc.setFrame(frame);
      } catch (_) {
        return;
      }
      stateGlobal.setMaximized(false);
    }
  }

  updateScreen() async {
    _screen = await _getCurrentScreen();
  }

  Future<window_size.Screen?> _getCurrentScreen() async {
    try {
      return (await window_size.getWindowInfo()).screen;
    } catch (e) {
      debugPrint('Failed to get current window screen: $e');
      return null;
    }
  }

  Future<bool> isWindowCanBeAdjusted([BuildContext? context]) async {
    // Capture the view before awaiting because the menu context may be disposed.
    final views = WidgetsBinding.instance.platformDispatcher.views;
    if (context == null && views.isEmpty) {
      return false;
    }
    final view = context != null ? View.of(context) : views.first;
    final mediaSize = MediaQueryData.fromView(view).size;
    final viewStyle =
        await bind.sessionGetViewStyle(sessionId: ffi.sessionId) ?? '';
    if (viewStyle != kRemoteViewStyleOriginal) {
      return false;
    }
    final remoteCount = RemoteCountState.find().value;
    if (remoteCount != 1) {
      return false;
    }
    await updateScreen();
    if (_screen == null) {
      return false;
    }
    return await _getAdjustedWindowFrame(mediaSize, forMenu: true) != null;
  }
}
