import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../consts.dart';
import '../models/model.dart';
import '../models/platform_model.dart';

import 'globals.dart';
import 'window_restore.dart';

Future<List<Rect>> getScreenListWayland() async {
  final screenRectList = <Rect>[];
  if (isMainDesktopWindow) {
    for (var screen in await window_size.getScreenList()) {
      final scale = kIgnoreDpi ? 1.0 : screen.scaleFactor;
      double l = screen.frame.left;
      double t = screen.frame.top;
      double r = screen.frame.right;
      double b = screen.frame.bottom;
      final rect = Rect.fromLTRB(l / scale, t / scale, r / scale, b / scale);
      screenRectList.add(rect);
    }
  } else {
    final screenList = await rustDeskWinManager.call(
        WindowType.Main, kWindowGetScreenList, '');
    try {
      for (var screen in jsonDecode(screenList.result) as List<dynamic>) {
        final scale = kIgnoreDpi ? 1.0 : screen['scaleFactor'];
        double l = screen['frame']['l'];
        double t = screen['frame']['t'];
        double r = screen['frame']['r'];
        double b = screen['frame']['b'];
        final rect = Rect.fromLTRB(l / scale, t / scale, r / scale, b / scale);
        screenRectList.add(rect);
      }
    } catch (e) {
      debugPrint('Failed to parse screenList: $e');
    }
  }
  return screenRectList;
}

Future<List<Rect>> getScreenListNotWayland() async {
  final screenRectList = <Rect>[];
  final displays = bind.mainGetDisplays();
  if (displays.isEmpty) {
    return screenRectList;
  }
  try {
    for (var display in jsonDecode(displays) as List<dynamic>) {
      // to-do: scale factor ?
      // final scale = kIgnoreDpi ? 1.0 : screen.scaleFactor;
      double l = display['x'].toDouble();
      double t = display['y'].toDouble();
      double r = (display['x'] + display['w']).toDouble();
      double b = (display['y'] + display['h']).toDouble();
      screenRectList.add(Rect.fromLTRB(l, t, r, b));
    }
  } catch (e) {
    debugPrint('Failed to parse displays: $e');
  }
  return screenRectList;
}

Future<List<Rect>> getScreenRectList() async {
  return bind.mainCurrentIsWayland()
      ? await getScreenListWayland()
      : await getScreenListNotWayland();
}

openMonitorInTheSameTab(int i, FFI ffi, PeerInfo pi,
    {bool updateCursorPos = true, bool recordSelection = true}) {
  if (recordSelection) {
    ffi.ffiModel.lastUserDisplay = i;
    ffi.ffiModel.cancelPendingRestoreTimer();
    ffi.ffiModel.pendingMonitorRestore = null;
  }
  final displays = i == kAllDisplayValue
      ? List.generate(pi.displays.length, (index) => index)
      : [i];
  // Try clear image model before switching from all displays
  // 1. The remote side has multiple displays.
  // 2. Do not use texture render.
  // 3. Connect to Display 1.
  // 4. Switch to multi-displays `kAllDisplayValue`
  // 5. Switch to Display 2.
  // Then the remote page will display last picture of Display 1 at the beginning.
  if (pi.forceTextureRender && i != kAllDisplayValue) {
    ffi.imageModel.clearImage();
  }
  bind.sessionSwitchDisplay(
    isDesktop: isDesktop,
    sessionId: ffi.sessionId,
    value: Int32List.fromList(displays),
  );
  ffi.ffiModel.switchToNewDisplay(i, ffi.sessionId, ffi.id,
      updateCursorPos: updateCursorPos);
}

// Open new tab or window to show this monitor.
// For now just open new window.
//
// screenRect is used to move the new window to the specified screen and set fullscreen.
openMonitorInNewTabOrWindow(int i, String peerId, PeerInfo pi,
    {Rect? screenRect}) {
  final args = {
    'window_id': stateGlobal.windowId,
    'peer_id': peerId,
    'display': i,
    'display_count': pi.displays.length,
    'window_type': (kWindowType ?? WindowType.RemoteDesktop).index,
  };
  if (screenRect != null) {
    args['screen_rect'] = {
      'l': screenRect.left,
      't': screenRect.top,
      'r': screenRect.right,
      'b': screenRect.bottom,
    };
  }
  DesktopMultiWindow.invokeMethod(
      kMainWindowId, kWindowEventOpenMonitorSession, jsonEncode(args));
}

setNewConnectWindowFrame(int windowId, String peerId, int preSessionCount,
    WindowType windowType, int? display, Rect? screenRect) async {
  if (screenRect == null) {
    // Do not restore window position to new connection if there's a pre-session.
    // https://github.com/rustdesk/rustdesk/discussions/8825
    if (preSessionCount == 0) {
      await restoreWindowPosition(windowType,
          windowId: windowId, display: display, peerId: peerId);
    }
  } else {
    await tryMoveToScreenAndSetFullscreen(screenRect);
  }
}

tryMoveToScreenAndSetFullscreen(Rect? screenRect) async {
  if (screenRect == null) {
    return;
  }
  final wc = WindowController.fromWindowId(stateGlobal.windowId);
  final curFrame = await wc.getFrame();
  final frame =
      Rect.fromLTWH(screenRect.left + 30, screenRect.top + 30, 600, 400);
  if (stateGlobal.fullscreen.isTrue &&
      curFrame.left <= frame.left &&
      curFrame.top <= frame.top &&
      curFrame.width >= frame.width &&
      curFrame.height >= frame.height) {
    return;
  }
  await wc.setFrame(frame);
  // An duration is needed to avoid the window being restored after fullscreen.
  Future.delayed(Duration(milliseconds: 300), () async {
    stateGlobal.setFullscreen(true);
  });
}

parseParamScreenRect(Map<String, dynamic> params) {
  Rect? screenRect;
  if (params['screen_rect'] != null) {
    double l = params['screen_rect']['l'];
    double t = params['screen_rect']['t'];
    double r = params['screen_rect']['r'];
    double b = params['screen_rect']['b'];
    screenRect = Rect.fromLTRB(l, t, r, b);
  }
  return screenRect;
}

get isInputSourceFlutter => stateGlobal.getInputSource() == "Input source 2";
