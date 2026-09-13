part of 'model.dart';

// Windows that already received their first-open frame; a second session
// opened as a tab in the same window keeps the frame.
final Set<int> _fittedWindows = {};

extension FfiModelWindowFit on FfiModel {
  /// On the first peer info of a remote-desktop window whose peer has no
  /// remembered frame, size the window to the remote's aspect ratio inside
  /// the work area (window_fit.dart) instead of the fixed 1280x720. Later
  /// resolution changes leave the window alone.
  Future<void> fitWindowToPeer(String peerId) async {
    if (!isDesktop || parent.target?.connType != ConnType.defaultConn) return;
    final windowId = stateGlobal.windowId;
    if (windowId < 0 || _fittedWindows.contains(windowId)) return;
    _fittedWindows.add(windowId);
    if (stateGlobal.fullscreen.isTrue) return;
    final remembered = bind.mainGetPeerFlutterOptionSync(
        id: peerId, k: windowFramePrefix + WindowType.RemoteDesktop.name);
    if (remembered.isNotEmpty) return;
    if (_pi.currentDisplay < 0 || _pi.currentDisplay >= _pi.displays.length) {
      return;
    }
    final display = _pi.displays[_pi.currentDisplay];
    try {
      final screen = (await window_size.getWindowInfo()).screen;
      if (screen == null) return;
      final frame = fitRemoteWindowFrame(
          Size(display.width / display.scale, display.height / display.scale),
          screen.visibleFrame);
      await WindowController.fromWindowId(windowId).setFrame(frame);
    } catch (e) {
      debugPrint('fitWindowToPeer: $e');
    }
  }
}
