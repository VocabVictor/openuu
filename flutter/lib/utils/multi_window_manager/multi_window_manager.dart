import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/window_fit.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:window_size/window_size.dart' as window_size;
part 'windows.dart';
part 'new_windows.dart';
part 'sessions.dart';

/// must keep the order
// ignore: constant_identifier_names
enum WindowType {
  Main,
  RemoteDesktop,
  FileTransfer,
  ViewCamera,
  PortForward,
  Terminal,
  Unknown
}

extension Index on int {
  WindowType get windowType {
    switch (this) {
      case 0:
        return WindowType.Main;
      case 1:
        return WindowType.RemoteDesktop;
      case 2:
        return WindowType.FileTransfer;
      case 3:
        return WindowType.ViewCamera;
      case 4:
        return WindowType.PortForward;
      case 5:
        return WindowType.Terminal;
      default:
        return WindowType.Unknown;
    }
  }
}

class MultiWindowCallResult {
  int windowId;
  dynamic result;

  MultiWindowCallResult(this.windowId, this.result);
}

/// Window Manager
/// mainly use it in `Main Window`
/// use it in sub window is not recommended
class RustDeskMultiWindowManager {
  RustDeskMultiWindowManager._();

  static final instance = RustDeskMultiWindowManager._();

  final Set<int> _inactiveWindows = {};
  final Set<int> _activeWindows = {};
  final List<AsyncCallback> _windowActiveCallbacks = List.empty(growable: true);
  final List<int> _remoteDesktopWindows = List.empty(growable: true);
  final List<int> _fileTransferWindows = List.empty(growable: true);
  final List<int> _viewCameraWindows = List.empty(growable: true);
  final List<int> _portForwardWindows = List.empty(growable: true);
  final List<int> _terminalWindows = List.empty(growable: true);

  moveTabToNewWindow(int windowId, String peerId, String sessionId,
      WindowType windowType) async {
    var params = {
      'type': windowType.index,
      'id': peerId,
      'tab_window_id': windowId,
      'session_id': sessionId,
    };
    if (windowType == WindowType.RemoteDesktop) {
      await _newSession(
        false,
        WindowType.RemoteDesktop,
        kWindowEventNewRemoteDesktop,
        peerId,
        _remoteDesktopWindows,
        jsonEncode(params),
      );
    } else if (windowType == WindowType.ViewCamera) {
      await _newSession(
        false,
        WindowType.ViewCamera,
        kWindowEventNewViewCamera,
        peerId,
        _viewCameraWindows,
        jsonEncode(params),
      );
    }
  }

  Future<MultiWindowCallResult> call(
      WindowType type, String methodName, dynamic args) async {
    final wnds = _findWindowsByType(type);
    if (wnds.isEmpty) {
      return MultiWindowCallResult(kInvalidWindowId, null);
    }
    for (final windowId in wnds) {
      if (_activeWindows.contains(windowId)) {
        final res =
            await DesktopMultiWindow.invokeMethod(windowId, methodName, args);
        return MultiWindowCallResult(windowId, res);
      }
    }
    final res =
        await DesktopMultiWindow.invokeMethod(wnds[0], methodName, args);
    return MultiWindowCallResult(wnds[0], res);
  }

  List<int> _findWindowsByType(WindowType type) {
    switch (type) {
      case WindowType.Main:
        return [kMainWindowId];
      case WindowType.RemoteDesktop:
        return _remoteDesktopWindows;
      case WindowType.FileTransfer:
        return _fileTransferWindows;
      case WindowType.ViewCamera:
        return _viewCameraWindows;
      case WindowType.PortForward:
        return _portForwardWindows;
      case WindowType.Terminal:
        return _terminalWindows;
      case WindowType.Unknown:
        break;
    }
    return [];
  }

  Set<int> getActiveWindows() {
    return _activeWindows;
  }

  Future<void> _notifyActiveWindow() async {
    for (final callback in _windowActiveCallbacks) {
      await callback.call();
    }
  }

  Future<void> registerActiveWindow(int windowId) async {
    _activeWindows.add(windowId);
    _inactiveWindows.remove(windowId);
    await _notifyActiveWindow();
  }

  /// Remove active window which has [`windowId`]
  ///
  /// [Availability]
  /// This function should only be called from main window.
  /// For other windows, please post a unregister(hide) event to main window handler:
  /// `rustDeskWinManager.call(WindowType.Main, kWindowEventHide, {"id": windowId!});`
  Future<void> unregisterActiveWindow(int windowId) async {
    _activeWindows.remove(windowId);
    if (windowId != kMainWindowId) {
      _inactiveWindows.add(windowId);
    }
    await _notifyActiveWindow();
  }

  void registerActiveWindowListener(AsyncCallback callback) {
    _windowActiveCallbacks.add(callback);
  }

  void unregisterActiveWindowListener(AsyncCallback callback) {
    _windowActiveCallbacks.remove(callback);
  }

  // This function is called from the main window.
  // It will query the active remote windows to get their coords.
  Future<List<String>> getOtherRemoteWindowCoords(int wId) async {
    List<String> coords = [];
    for (final windowId in _remoteDesktopWindows) {
      if (windowId != wId) {
        if (_activeWindows.contains(windowId)) {
          final res = await DesktopMultiWindow.invokeMethod(
              windowId, kWindowEventRemoteWindowCoords, '');
          if (res != null) {
            coords.add(res);
          }
        }
      }
    }
    return coords;
  }

  // This function is called from one remote window.
  // Only the main window knows `_remoteDesktopWindows` and `_activeWindows`.
  // So we need to call the main window to get the other remote windows' coords.
  Future<List<RemoteWindowCoords>> getOtherRemoteWindowCoordsFromMain() async {
    List<RemoteWindowCoords> coords = [];
    // Call the main window to get the coords of other remote windows.
    String res = await DesktopMultiWindow.invokeMethod(
        kMainWindowId, kWindowEventRemoteWindowCoords, kWindowId.toString());
    List<dynamic> list = jsonDecode(res);
    for (var item in list) {
      coords.add(RemoteWindowCoords.fromJson(jsonDecode(item)));
    }
    return coords;
  }
}

final rustDeskWinManager = RustDeskMultiWindowManager.instance;
