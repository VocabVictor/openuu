import 'dart:async';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/refresh_wrapper.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/utils/platform_channel.dart';
import 'package:get/get.dart';
import 'package:wakelock_plus/wakelock_plus.dart';
import 'package:window_manager/window_manager.dart';

import '../../consts.dart';
import '../models/platform_model.dart';

import 'package:flutter_hbb/native/win32.dart'
    if (dart.library.html) 'package:flutter_hbb/web/win32.dart';
import 'ffi_options.dart';
import 'globals.dart';
import 'my_theme.dart';
import 'window_position.dart';

/// only available for Windows target
int windowsBuildNumber = 0;

DesktopType? desktopType;

Future<void> windowOnTop(int? id) async {
  if (!isDesktop) {
    return;
  }
  print("Bring window '$id' on top");
  if (id == null) {
    // main window
    if (stateGlobal.isMinimized) {
      await windowManager.restore();
    }
    await windowManager.show();
    await windowManager.focus();
    await rustDeskWinManager.registerActiveWindow(kWindowMainId);
  } else {
    WindowController.fromWindowId(id)
      ..focus()
      ..show();
    rustDeskWinManager.call(WindowType.Main, kWindowEventShow, {"id": id});
  }
}

/// Wakelock manager with reference counting for desktop.
/// Ensures wakelock is only disabled when all sessions are closed/minimized.
///
/// Note: Each isolate has its own WakelockPlus instance with independent assertion.
/// As long as one isolate has wakelock enabled, the screen stays awake.
/// This manager handles multiple tabs within the same isolate.
class WakelockManager {
  static final Set<UniqueKey> _enabledKeys = {};
  // Don't use WakelockPlus.enabled, it causes error on Android:
  // Unhandled Exception: FormatException: Message corrupted
  //
  // On Linux, multiple enable() calls create only one inhibit, but each disable()
  // only releases if _cookie != null. So we need our own _enabled state to avoid
  // calling disable() when not enabled.
  // See: https://github.com/fluttercommunity/wakelock_plus/blob/0c74e5bbc6aefac57b6c96bb7ef987705ed559ec/wakelock_plus/lib/src/wakelock_plus_linux_plugin.dart#L48
  static bool _enabled = false;

  static void enable(UniqueKey key, {bool isServer = false}) {
    // Check if we should keep awake during outgoing sessions
    if (!isServer) {
      final keepAwake =
          mainGetLocalBoolOptionSync(kOptionKeepAwakeDuringOutgoingSessions);
      if (!keepAwake) {
        return; // Don't enable wakelock if user disabled keep awake
      }
    }
    if (isDesktop) {
      _enabledKeys.add(key);
    }
    if (!_enabled) {
      _enabled = true;
      WakelockPlus.enable();
    }
  }

  static void disable(UniqueKey key) {
    if (isDesktop) {
      _enabledKeys.remove(key);
      if (_enabledKeys.isNotEmpty) {
        return;
      }
    }
    if (_enabled) {
      WakelockPlus.disable();
      _enabled = false;
    }
  }
}

/// call this to reload current window.
///
/// [Note]
/// Must have [RefreshWrapper] on the top of widget tree.
void reloadCurrentWindow() {
  if (Get.context != null) {
    // reload self window
    RefreshWrapper.of(Get.context!)?.rebuild();
  } else {
    debugPrint(
        "reload current window failed, global BuildContext does not exist");
  }
}

/// call this to reload all windows, including main + all sub windows.
Future<void> reloadAllWindows() async {
  reloadCurrentWindow();
  try {
    final ids = await DesktopMultiWindow.getAllSubWindowIds();
    for (final id in ids) {
      DesktopMultiWindow.invokeMethod(id, kWindowActionRebuild);
    }
  } on AssertionError {
    // ignore
  }
}

/// Indicate the flutter app is running in portable mode.
///
/// [Note]
/// Portable build is only available on Windows.
bool isRunningInPortableMode() {
  if (!isWindows) {
    return false;
  }
  return bool.hasEnvironment(kEnvPortableExecutable);
}

/// Window status callback
Future<void> onActiveWindowChanged() async {
  print(
      "[MultiWindowHandler] active window changed: ${rustDeskWinManager.getActiveWindows()}");
  if (rustDeskWinManager.getActiveWindows().isEmpty) {
    // close all sub windows
    try {
      if (isLinux) {
        await Future.wait([
          saveWindowPosition(WindowType.Main),
          rustDeskWinManager.closeAllSubWindows()
        ]);
      } else {
        await rustDeskWinManager.closeAllSubWindows();
      }
    } catch (err) {
      debugPrintStack(label: "$err");
    } finally {
      debugPrint("Start closing OpenUU...");
      await windowManager.setPreventClose(false);
      await windowManager.close();
      if (isMacOS) {
        // If we call without delay, `flutter/macos/Runner/MainFlutterWindow.swift` can handle the "terminate" event.
        // But the app will not close.
        //
        // No idea why we need to delay here, `terminate()` itself is also an async function.
        //
        // A quick workaround, use `Timer.periodic` to avoid the app not closing.
        // Because `await windowManager.close()` and `RdPlatformChannel.instance.terminate()`
        // may not work since `Flutter 3.24.4`, see the following logs.
        // A delay will allow the app to close.
        //
        //```
        // embedder.cc (2725): 'FlutterPlatformMessageCreateResponseHandle' returned 'kInvalidArguments'. Engine handle was invalid.
        // 2024-11-11 11:41:11.546 RustDesk[90272:2567686] Failed to create a FlutterPlatformMessageResponseHandle (2)
        // embedder.cc (2672): 'FlutterEngineSendPlatformMessage' returned 'kInvalidArguments'. Invalid engine handle.
        // 2024-11-11 11:41:11.565 RustDesk[90272:2567686] Failed to send message to Flutter engine on channel 'flutter/lifecycle' (2).
        // ```
        periodic_immediate(
            Duration(milliseconds: 30), RdPlatformChannel.instance.terminate);
      }
    }
  }
}

Timer periodic_immediate(Duration duration, Future<void> Function() callback) {
  Future.delayed(Duration.zero, callback);
  return Timer.periodic(duration, (timer) async {
    await callback();
  });
}

/// return a human readable windows version
WindowsTarget getWindowsTarget(int buildNumber) {
  if (!isWindows) {
    return WindowsTarget.naw;
  }
  if (buildNumber >= 22000) {
    return WindowsTarget.w11;
  } else if (buildNumber >= 10240) {
    return WindowsTarget.w10;
  } else if (buildNumber >= 9600) {
    return WindowsTarget.w8_1;
  } else if (buildNumber >= 9200) {
    return WindowsTarget.w8;
  } else if (buildNumber >= 7601) {
    return WindowsTarget.w7;
  } else if (buildNumber >= 6002) {
    return WindowsTarget.vista;
  } else {
    // minimum support
    return WindowsTarget.xp;
  }
}

/// Get windows target build number.
///
/// [Note]
/// Please use this function wrapped with `Platform.isWindows`.
int getWindowsTargetBuildNumber() {
  return getWindowsTargetBuildNumber_();
}

/// Indicating we need to use compatible ui mode.
///
/// [Conditions]
/// - Windows 7, window will overflow when we use frameless ui.
bool get kUseCompatibleUiMode =>
    isWindows &&
    const [WindowsTarget.w7].contains(windowsBuildNumber.windowsVersion);

bool get isWin10 => windowsBuildNumber.windowsVersion == WindowsTarget.w10;

String getWindowName({WindowType? overrideType}) {
  final name = bind.mainGetAppNameSync();
  switch (overrideType ?? kWindowType) {
    case WindowType.Main:
      return name;
    case WindowType.FileTransfer:
      return "File Transfer - $name";
    case WindowType.ViewCamera:
      return "View Camera - $name";
    case WindowType.PortForward:
      return "Port Forward - $name";
    case WindowType.RemoteDesktop:
      return "Remote Desktop - $name";
    default:
      break;
  }
  return name;
}

String getWindowNameWithId(String id, {WindowType? overrideType}) {
  return "${DesktopTab.tablabelGetter(id).value} - ${getWindowName(overrideType: overrideType)}";
}

Future<void> updateSystemWindowTheme() async {
  // Set system window theme for macOS.
  final userPreference = MyTheme.getThemeModePreference();
  if (userPreference != ThemeMode.system) {
    if (isMacOS) {
      await RdPlatformChannel.instance.changeSystemWindowTheme(
          userPreference == ThemeMode.light
              ? SystemWindowTheme.light
              : SystemWindowTheme.dark);
    }
  }
}
