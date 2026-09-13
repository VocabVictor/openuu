import 'dart:async';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:window_manager/window_manager.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../../consts.dart';
import '../models/platform_model.dart';

import 'globals.dart';
import 'window_position.dart';

Future<Size> _adjustRestoreMainWindowSize(double? width, double? height) async {
  const double minWidth = 1;
  const double minHeight = 1;
  const double maxWidth = 6480;
  const double maxHeight = 6480;

  final defaultWidth =
      (isDesktop ? 1280 : kMobileDefaultDisplayWidth)
          .toDouble();
  final defaultHeight =
      (isDesktop ? 720 : kMobileDefaultDisplayHeight)
          .toDouble();
  double restoreWidth = width ?? defaultWidth;
  double restoreHeight = height ?? defaultHeight;

  if (restoreWidth < minWidth) {
    restoreWidth = defaultWidth;
  }
  if (restoreHeight < minHeight) {
    restoreHeight = defaultHeight;
  }
  if (restoreWidth > maxWidth) {
    restoreWidth = defaultWidth;
  }
  if (restoreHeight > maxHeight) {
    restoreHeight = defaultHeight;
  }
  return Size(restoreWidth, restoreHeight);
}

// Consider using Rect.contains() instead,
// though the implementation is not exactly the same.
bool isPointInRect(Offset point, Rect rect) {
  return point.dx >= rect.left &&
      point.dx <= rect.right &&
      point.dy >= rect.top &&
      point.dy <= rect.bottom;
}

/// return null means center
Future<Offset?> _adjustRestoreMainWindowOffset(
  double? left,
  double? top,
  double? width,
  double? height,
) async {
  if (left == null || top == null || width == null || height == null) {
    return null;
  }

  if (isDesktop) {
    final screens = await window_size.getScreenList();
    if (screens.isNotEmpty) {
      final windowRect = Rect.fromLTWH(left, top, width, height);
      bool isVisible = false;
      for (final screen in screens) {
        final intersection = windowRect.intersect(screen.visibleFrame);
        if (intersection.width >= 10.0 && intersection.height >= 10.0) {
          isVisible = true;
          break;
        }
      }
      if (!isVisible) {
        return null;
      }
      return Offset(left, top);
    }
  }

  double frameLeft = 0.0;
  double frameTop = 0.0;
  double frameRight = (isDesktop ? kDesktopMaxDisplaySize
          : kMobileMaxDisplaySize)
      .toDouble();
  double frameBottom = (isDesktop ? kDesktopMaxDisplaySize
          : kMobileMaxDisplaySize)
      .toDouble();

  final minWidth = 10.0;
  if ((left + minWidth) > frameRight ||
      (top + minWidth) > frameBottom ||
      (left + width - minWidth) < frameLeft ||
      top < frameTop) {
    return null;
  } else {
    return Offset(left, top);
  }
}

/// Restore window position and size on start
/// Note that windowId must be provided if it's subwindow
//
// display is used to set the offset of the window in individual display mode.
Future<bool> restoreWindowPosition(WindowType type,
    {int? windowId, String? peerId, int? display}) async {
  if (bind
      .mainGetEnv(key: "DISABLE_RUSTDESK_RESTORE_WINDOW_POSITION")
      .isNotEmpty) {
    return false;
  }
  if (type != WindowType.Main && windowId == null) {
    debugPrint(
        "Error: windowId cannot be null when saving positions for sub window");
    return false;
  }

  bool isRemotePeerPos = false;
  String? pos;
  // No need to check mainGetLocalBoolOptionSync(kOptionOpenNewConnInTabs)
  // Though "open in tabs" is true and the new window restore peer position, it's ok.
  if ((type == WindowType.RemoteDesktop || type == WindowType.ViewCamera) &&
      windowId != null &&
      peerId != null) {
    final peerPos = bind.mainGetPeerFlutterOptionSync(
        id: peerId, k: windowFramePrefix + type.name);
    if (peerPos.isNotEmpty) {
      pos = peerPos;
    }
    isRemotePeerPos = pos != null;
  }
  // A remote-desktop window without a frame of its own peer keeps the frame it
  // was created with and is fitted to the peer's display on the first peer
  // info (fitWindowToPeer); another peer's remembered frame, maximized or not,
  // does not apply to it.
  if (type == WindowType.RemoteDesktop && !isRemotePeerPos) {
    return false;
  }
  pos ??= bind.getLocalFlutterOption(k: windowFramePrefix + type.name);

  var lpos = LastWindowPosition.loadFromString(pos);
  if (lpos == null) {
    debugPrint("No window position saved, trying to center the window.");
    switch (type) {
      case WindowType.Main:
        // Center the main window only if no position is saved (on first run).
        if (isWindows || isLinux) {
          await windowManager.center();
        }
        // For MacOS, the window is already centered by default.
        // See https://github.com/rustdesk/rustdesk/blob/9b9276e7524523d7f667fefcd0694d981443df0e/flutter/macos/Runner/Base.lproj/MainMenu.xib#L333
        // If `<windowPositionMask>` in `<window>` is not set, the window will be centered.
        break;
      default:
        // No need to change the position of a sub window if no position is saved,
        // since the default position is already centered.
        // https://github.com/rustdesk/rustdesk/blob/317639169359936f7f9f85ef445ec9774218772d/flutter/lib/utils/multi_window_manager.dart#L163
        break;
    }
    return true;
  }
  if (type == WindowType.RemoteDesktop || type == WindowType.ViewCamera) {
    if (!isRemotePeerPos && windowId != null) {
      if (lpos.offsetWidth != null) {
        lpos.offsetWidth = lpos.offsetWidth! + windowId * kNewWindowOffset;
      }
      if (lpos.offsetHeight != null) {
        lpos.offsetHeight = lpos.offsetHeight! + windowId * kNewWindowOffset;
      }
    }
    if (display != null) {
      if (lpos.offsetWidth != null) {
        lpos.offsetWidth = lpos.offsetWidth! + display * kNewWindowOffset;
      }
      if (lpos.offsetHeight != null) {
        lpos.offsetHeight = lpos.offsetHeight! + display * kNewWindowOffset;
      }
    }
  }

  final size = await _adjustRestoreMainWindowSize(lpos.width, lpos.height);
  final offsetLeftTop = await _adjustRestoreMainWindowOffset(
    lpos.offsetWidth,
    lpos.offsetHeight,
    size.width,
    size.height,
  );
  debugPrint(
      "restore lpos: ${size.width}/${size.height}, offset:${offsetLeftTop?.dx}/${offsetLeftTop?.dy}, isMaximized: ${lpos.isMaximized}, isFullscreen: ${lpos.isFullscreen}");

  switch (type) {
    case WindowType.Main:
      restorePos() async {
        if (offsetLeftTop == null) {
          await windowManager.center();
        } else {
          await windowManager.setPosition(offsetLeftTop,
              ignoreDevicePixelRatio: ignoreDevicePixelRatio);
        }
      }
      if (lpos.isMaximized == true) {
        await restorePos();
        if (!(bind.isIncomingOnly() || bind.isOutgoingOnly())) {
          await windowManager.maximize();
        }
      } else {
        final storeSize = !bind.isIncomingOnly() || bind.isOutgoingOnly();
        if (isWindows) {
          if (storeSize) {
            // We need to set the window size first to avoid the incorrect size in some special cases.
            // E.g. There are two monitors, the left one is 100% DPI and the right one is 175% DPI.
            // The window belongs to the left monitor, but if it is moved a little to the right, it will belong to the right monitor.
            // After restoring, the size will be incorrect.
            // See known issue in https://github.com/rustdesk/rustdesk/pull/9840
            await windowManager.setSize(size,
                ignoreDevicePixelRatio: ignoreDevicePixelRatio);
          }
          await restorePos();
          if (storeSize) {
            await windowManager.setSize(size,
                ignoreDevicePixelRatio: ignoreDevicePixelRatio);
          }
        } else {
          if (storeSize) {
            await windowManager.setSize(size,
                ignoreDevicePixelRatio: ignoreDevicePixelRatio);
          }
          await restorePos();
        }
      }
      return true;
    default:
      final wc = WindowController.fromWindowId(windowId!);
      restoreFrame() async {
        if (offsetLeftTop == null) {
          await wc.center();
        } else {
          final frame = Rect.fromLTWH(
              offsetLeftTop.dx, offsetLeftTop.dy, size.width, size.height);
          await wc.setFrame(frame);
        }
      }
      if (lpos.isFullscreen == true) {
        if (!isMacOS) {
          await restoreFrame();
        }
        // An duration is needed to avoid the window being restored after fullscreen.
        Future.delayed(Duration(milliseconds: 300), () async {
          if (kWindowId == windowId) {
            stateGlobal.setFullscreen(true);
          } else {
            // If is not current window, we need to send a fullscreen message to `windowId`
            DesktopMultiWindow.invokeMethod(
                windowId, kWindowEventSetFullscreen, 'true');
          }
        });
      } else if (lpos.isMaximized == true) {
        await restoreFrame();
        // An duration is needed to avoid the window being restored after maximized.
        Future.delayed(Duration(milliseconds: 300), () async {
          await wc.maximize();
        });
      } else {
        await restoreFrame();
      }
      break;
  }
  return false;
}
