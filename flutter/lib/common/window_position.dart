import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/window_fit.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:get/get_rx/src/rx_workers/utils/debouncer.dart';
import 'package:window_manager/window_manager.dart';

import '../consts.dart';
import '../models/platform_model.dart';

import 'globals.dart';

// Only used on Windows(window manager).
bool ignoreDevicePixelRatio = true;

class LastWindowPosition {
  double? width;
  double? height;
  double? offsetWidth;
  double? offsetHeight;
  bool? isMaximized;
  bool? isFullscreen;

  LastWindowPosition(this.width, this.height, this.offsetWidth,
      this.offsetHeight, this.isMaximized, this.isFullscreen);

  bool equals(LastWindowPosition other) {
    return ((width == other.width) &&
        (height == other.height) &&
        (offsetWidth == other.offsetWidth) &&
        (offsetHeight == other.offsetHeight) &&
        (isMaximized == other.isMaximized) &&
        (isFullscreen == other.isFullscreen));
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      "width": width,
      "height": height,
      "offsetWidth": offsetWidth,
      "offsetHeight": offsetHeight,
      "isMaximized": isMaximized,
      "isFullscreen": isFullscreen,
    };
  }

  @override
  String toString() {
    return jsonEncode(toJson());
  }

  static LastWindowPosition? loadFromString(String content) {
    if (content.isEmpty) {
      return null;
    }
    try {
      final m = jsonDecode(content);
      return LastWindowPosition(m["width"], m["height"], m["offsetWidth"],
          m["offsetHeight"], m["isMaximized"], m["isFullscreen"]);
    } catch (e) {
      debugPrintStack(
          label:
              'Failed to load LastWindowPosition "$content" ${e.toString()}');
      return null;
    }
  }
}

String get windowFramePrefix =>
    kWindowPrefix +
    (bind.isIncomingOnly()
        ? "incoming_"
        : (bind.isOutgoingOnly() ? "outgoing_" : ""));

typedef WindowKey = ({WindowType type, int? windowId});

LastWindowPosition? _lastWindowPosition = null;

final Debouncer _saveWindowDebounce = Debouncer(delay: Duration(seconds: 1));

/// Save window position and size on exit
/// Note that windowId must be provided if it's subwindow
Future<void> saveWindowPosition(WindowType type,
    {int? windowId, bool? flush}) async {
  if (type != WindowType.Main && windowId == null) {
    debugPrint(
        "Error: windowId cannot be null when saving positions for sub window");
  }

  Offset? position;
  Size? sz;
  late bool isMaximized;
  bool isFullscreen = stateGlobal.fullscreen.isTrue;

  setPreFrame() {
    final pos = bind.getLocalFlutterOption(k: windowFramePrefix + type.name);
    var lpos = LastWindowPosition.loadFromString(pos);
    if (lpos != null) {
      if (lpos.offsetWidth != null && lpos.offsetHeight != null) {
        position = Offset(lpos.offsetWidth!, lpos.offsetHeight!);
      }
      if (lpos.width != null && lpos.height != null) {
        sz = Size(lpos.width!, lpos.height!);
      }
    }
  }

  switch (type) {
    case WindowType.Main:
      // Checking `bind.isIncomingOnly()` is a simple workaround for MacOS.
      // `await windowManager.isMaximized()` will always return true
      // if is not resizable. The reason is unknown.
      //
      // `setResizable(!bind.isIncomingOnly());` in main.dart
      isMaximized =
          bind.isIncomingOnly() ? false : await windowManager.isMaximized();
      if (isFullscreen || isMaximized) {
        setPreFrame();
      } else {
        position = await windowManager.getPosition(
            ignoreDevicePixelRatio: ignoreDevicePixelRatio);
        sz = await windowManager.getSize(
            ignoreDevicePixelRatio: ignoreDevicePixelRatio);
      }
      break;
    default:
      final wc = WindowController.fromWindowId(windowId!);
      isMaximized = await wc.isMaximized();
      if (isFullscreen || isMaximized) {
        setPreFrame();
      } else {
        final Rect frame;
        try {
          frame = await wc.getFrame();
        } catch (e) {
          debugPrint(
              "Failed to get frame of window $windowId, it may be hidden");
          return;
        }
        position = frame.topLeft;
        sz = frame.size;
      }
      break;
  }
  if (isWindows && position != null) {
    const kMinOffset = -10000;
    const kMaxOffset = 10000;
    if (position!.dx < kMinOffset ||
        position!.dy < kMinOffset ||
        position!.dx > kMaxOffset ||
        position!.dy > kMaxOffset) {
      debugPrint("Invalid position: $position, ignore saving position");
      return;
    }
  }

  final pos = LastWindowPosition(sz?.width, sz?.height, position?.dx,
      position?.dy, isMaximized, isFullscreen);

  final WindowKey key = (type: type, windowId: windowId);

  final bool haveNewWindowPosition =
      (_lastWindowPosition == null) || !pos.equals(_lastWindowPosition!);
  final bool isPreviousNewWindowPositionPending = _saveWindowDebounce.isRunning;

  if (haveNewWindowPosition || isPreviousNewWindowPositionPending) {
    _lastWindowPosition = pos;

    if (flush ?? false) {
      // If a previous update is pending, replace it.
      _saveWindowDebounce.cancel();
      await _saveWindowPositionActual(key);
    } else if (haveNewWindowPosition) {
      _saveWindowDebounce.call(() => _saveWindowPositionActual(key));
    }
  }
}

Future<void> _saveWindowPositionActual(WindowKey key) async {
  LastWindowPosition? pos = _lastWindowPosition;

  if (pos != null) {
    debugPrint(
        "Saving frame: ${key.windowId}: ${pos.width}/${pos.height}, offset:${pos.offsetWidth}/${pos.offsetHeight}, isMaximized:${pos.isMaximized}, isFullscreen:${pos.isFullscreen}");

    await bind.setLocalFlutterOption(
        k: windowFramePrefix + key.type.name, v: pos.toString());

    final userSized =
        key.type != WindowType.RemoteDesktop || sessionWindowUserSized;
    if ((key.type == WindowType.RemoteDesktop ||
            key.type == WindowType.ViewCamera) &&
        key.windowId != null &&
        userSized) {
      await _saveSessionWindowPosition(key.type, key.windowId!,
          pos.isMaximized ?? false, pos.isFullscreen ?? false, pos);
    }
  }
}

Future _saveSessionWindowPosition(WindowType windowType, int windowId,
    bool isMaximized, bool isFullscreen, LastWindowPosition pos) async {
  final remoteList = await DesktopMultiWindow.invokeMethod(
      windowId, kWindowEventGetRemoteList, null);
  getPeerPos(String peerId) {
    if (isMaximized || isFullscreen) {
      final peerPos = bind.mainGetPeerFlutterOptionSync(
          id: peerId, k: windowFramePrefix + windowType.name);
      var lpos = LastWindowPosition.loadFromString(peerPos);
      // A maximized window keeps the size it had before, so restoring it
              // does not open a full-screen one; without a stored size the
              // current one is that size, not the window's offset.
      return LastWindowPosition(
              lpos?.width ?? pos.width,
              lpos?.height ?? pos.height,
              lpos?.offsetWidth ?? pos.offsetWidth,
              lpos?.offsetHeight ?? pos.offsetHeight,
              isMaximized,
              isFullscreen)
          .toString();
    } else {
      return pos.toString();
    }
  }

  if (remoteList != null) {
    for (final peerId in remoteList.split(',')) {
      bind.mainSetPeerFlutterOptionSync(
          id: peerId,
          k: windowFramePrefix + windowType.name,
          v: getPeerPos(peerId));
    }
  }
}
