import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:window_manager/window_manager.dart';

import '../../consts.dart';
import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'globals.dart';
import 'widgets_misc.dart';
import 'package:flutter_hbb/models/model.dart';

/// Flutter can't not catch PointerMoveEvent when size is 1
/// This will happen in Android AccessibilityService Input
/// android can't init dispatching size yet ,see: https://stackoverflow.com/questions/59960451/android-accessibility-dispatchgesture-is-it-possible-to-specify-pressure-for-a
/// use this temporary solution until flutter or android fixes the bug
class AccessibilityListener extends StatelessWidget {
  final Widget? child;
  static final offset = 100;

  AccessibilityListener({this.child});

  @override
  Widget build(BuildContext context) {
    return Listener(
        onPointerDown: (evt) {
          if (evt.size == 1) {
            GestureBinding.instance.handlePointerEvent(PointerAddedEvent(
                pointer: evt.pointer + offset, position: evt.position));
            GestureBinding.instance.handlePointerEvent(PointerDownEvent(
                pointer: evt.pointer + offset,
                size: 0.1,
                position: evt.position));
          }
        },
        onPointerUp: (evt) {
          if (evt.size == 1) {
            GestureBinding.instance.handlePointerEvent(PointerUpEvent(
                pointer: evt.pointer + offset,
                size: 0.1,
                position: evt.position));
            GestureBinding.instance.handlePointerEvent(PointerRemovedEvent(
                pointer: evt.pointer + offset, position: evt.position));
          }
        },
        onPointerMove: (evt) {
          if (evt.size == 1) {
            GestureBinding.instance.handlePointerEvent(PointerMoveEvent(
                pointer: evt.pointer + offset,
                size: 0.1,
                delta: evt.delta,
                position: evt.position));
          }
        },
        child: child);
  }
}

class AndroidPermissionManager {
  static Completer<bool>? _completer;
  static Timer? _timer;
  static var _current = "";

  static Future<bool> check(String type) {
    if (isDesktop) {
      return Future.value(true);
    }
    return gFFI.invokeMethod("check_permission", type);
  }

  // startActivity goto Android Setting's page to request permission manually by user
  static void startAction(String action) {
    gFFI.invokeMethod(AndroidChannel.kStartAction, action);
  }

  /// We use XXPermissions to request permissions,
  /// for supported types, see https://github.com/getActivity/XXPermissions/blob/e46caea32a64ad7819df62d448fb1c825481cd28/library/src/main/java/com/hjq/permissions/Permission.java
  static Future<bool> request(String type) {
    if (isDesktop) {
      return Future.value(true);
    }

    gFFI.invokeMethod("request_permission", type);

    // clear last task
    if (_completer?.isCompleted == false) {
      _completer?.complete(false);
    }
    _timer?.cancel();

    _current = type;
    _completer = Completer<bool>();

    _timer = Timer(Duration(seconds: 120), () {
      if (_completer == null) return;
      if (!_completer!.isCompleted) {
        _completer!.complete(false);
      }
      _completer = null;
      _current = "";
    });
    return _completer!.future;
  }

  static complete(String type, bool res) {
    if (type != _current) {
      res = false;
    }
    _timer?.cancel();
    _completer?.complete(res);
    _current = "";
  }
}

/// macOS only
///
/// Note: not found a general solution for rust based AVFoundation bingding.
/// [AVFoundation] crate has compile error.
const kMacOSPermChannel = MethodChannel("org.rustdesk.rustdesk/host");

enum PermissionAuthorizeType {
  undetermined,
  authorized,
  denied, // and restricted
}

Future<PermissionAuthorizeType> osxCanRecordAudio() async {
  int res = await kMacOSPermChannel.invokeMethod("canRecordAudio");
  print(res);
  if (res > 0) {
    return PermissionAuthorizeType.authorized;
  } else if (res == 0) {
    return PermissionAuthorizeType.undetermined;
  } else {
    return PermissionAuthorizeType.denied;
  }
}

Future<bool> osxRequestAudio() async {
  return await kMacOSPermChannel.invokeMethod("requestRecordAudio");
}

Widget futureBuilder(
    {required Future? future, required Widget Function(dynamic data) hasData}) {
  return FutureBuilder(
      future: future,
      builder: (BuildContext context, AsyncSnapshot snapshot) {
        if (snapshot.hasData) {
          return hasData(snapshot.data!);
        } else {
          if (snapshot.hasError) {
            debugPrint(snapshot.error.toString());
          }
          return Container();
        }
      });
}

void onCopyFingerprint(String value) {
  if (value.isNotEmpty) {
    Clipboard.setData(ClipboardData(text: value));
    showToast('$value\n${translate("Copied")}');
  } else {
    showToast(translate("no fingerprints"));
  }
}

void onCopyId(String value) {
  if (value.isNotEmpty) {
    Clipboard.setData(ClipboardData(text: value));
    showToast('$value\n${translate("Copied")}');
  } else {
    showToast(translate("Invalid ID"));
  }
}

Future<bool> callMainCheckSuperUserPermission() async {
  bool checked = await bind.mainCheckSuperUserPermission();
  if (isMacOS) {
    await windowManager.show();
  }
  return checked;
}

Future<void> start_service(bool is_start) async {
  bool checked = !bind.mainIsInstalled() ||
      !isMacOS ||
      await callMainCheckSuperUserPermission();
  if (checked) {
    mainSetBoolOption(kOptionStopService, !is_start);
  }
}
