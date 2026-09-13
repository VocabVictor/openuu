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
    if (windowId < 0 || _fittedWindows.contains(windowId)) {
      debugPrint('fitWindowToPeer: skip, window $windowId already fitted');
      return;
    }
    _fittedWindows.add(windowId);
    if (stateGlobal.fullscreen.isTrue) return;
    final remembered = bind.mainGetPeerFlutterOptionSync(
        id: peerId, k: windowFramePrefix + WindowType.RemoteDesktop.name);
    if (remembered.isNotEmpty) {
      sessionWindowUserSized = true;
      debugPrint('fitWindowToPeer: peer $peerId keeps its remembered frame');
      return;
    }
    if (_pi.currentDisplay < 0 || _pi.currentDisplay >= _pi.displays.length) {
      debugPrint('fitWindowToPeer: no current display yet');
      return;
    }
    final display = _pi.displays[_pi.currentDisplay];
    try {
      final screen = (await window_size.getWindowInfo()).screen;
      if (screen == null) {
        debugPrint('fitWindowToPeer: no screen info');
        return;
      }
      final work = logicalWorkArea(screen.visibleFrame, screen.scaleFactor);
      final frame = fitRemoteWindowFrame(
          Size(display.width / display.scale, display.height / display.scale),
          work);
      await WindowController.fromWindowId(windowId).setFrame(frame);
      sessionWindowFittedAt = DateTime.now();
      debugPrint(
          'fitWindowToPeer: ${display.width}x${display.height} in ${work.width}x${work.height} -> ${frame.width}x${frame.height} at ${frame.left},${frame.top}');
    } catch (e) {
      debugPrint('fitWindowToPeer: $e');
    }
  }
}
