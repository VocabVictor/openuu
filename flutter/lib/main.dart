import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:bot_toast/bot_toast.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/widgets/overlay.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';
import 'package:flutter_hbb/desktop/pages/install_page.dart';
import 'package:flutter_hbb/desktop/pages/server_page.dart';
import 'package:flutter_hbb/desktop/screen/desktop_file_transfer_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_view_camera_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_port_forward_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_remote_screen.dart';
import 'package:flutter_hbb/desktop/screen/desktop_terminal_screen.dart';
import 'package:flutter_hbb/desktop/widgets/refresh_wrapper.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';

import 'common.dart';
import 'consts.dart';
import 'mobile/pages/home_page.dart';
import 'mobile/pages/server_page.dart';
import 'mobile/widgets/deploy_dialog.dart';
import 'models/platform_model.dart';
import 'package:flutter_hbb/models/ab_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
part 'main/windows.dart';

/// Basic window and launch properties.
int? kWindowId;
WindowType? kWindowType;
late List<String> kBootArgs;

Future<void> main(List<String> args) async {
  earlyAssert();
  WidgetsFlutterBinding.ensureInitialized();

  debugPrint("launch args: $args");
  kBootArgs = List.from(args);

  if (!isDesktop) {
    runMobileApp();
    return;
  }
  // main window
  if (args.isNotEmpty && args.first == 'multi_window') {
    kWindowId = int.parse(args[1]);
    stateGlobal.setWindowId(kWindowId!);
    if (!isMacOS) {
      WindowController.fromWindowId(kWindowId!).showTitleBar(false);
    }
    final argument = args[2].isEmpty
        ? <String, dynamic>{}
        : jsonDecode(args[2]) as Map<String, dynamic>;
    int type = argument['type'] ?? -1;
    // to-do: No need to parse window id ?
    // Because stateGlobal.windowId is a global value.
    argument['windowId'] = kWindowId;
    kWindowType = type.windowType;
    switch (kWindowType) {
      case WindowType.RemoteDesktop:
        desktopType = DesktopType.remote;
        runMultiWindow(
          argument,
          kAppTypeDesktopRemote,
        );
        break;
      case WindowType.FileTransfer:
        desktopType = DesktopType.fileTransfer;
        runMultiWindow(
          argument,
          kAppTypeDesktopFileTransfer,
        );
        break;
      case WindowType.ViewCamera:
        desktopType = DesktopType.viewCamera;
        runMultiWindow(
          argument,
          kAppTypeDesktopViewCamera,
        );
        break;
      case WindowType.PortForward:
        desktopType = DesktopType.portForward;
        runMultiWindow(
          argument,
          kAppTypeDesktopPortForward,
        );
        break;
      case WindowType.Terminal:
        desktopType = DesktopType.terminal;
        runMultiWindow(
          argument,
          kAppTypeDesktopTerminal,
        );
      default:
        break;
    }
  } else if (args.isNotEmpty && args.first == '--cm') {
    debugPrint("--cm started");
    desktopType = DesktopType.cm;
    await windowManager.ensureInitialized();
    runConnectionManagerScreen();
  } else if (args.contains('--install')) {
    runInstallPage();
  } else {
    desktopType = DesktopType.main;
    await windowManager.ensureInitialized();
    windowManager.setPreventClose(true);
    if (isMacOS) {
      disableWindowMovable(kWindowId);
    }
    runMainApp(true);
  }
}

Future<void> initEnv(String appType) async {
  // global shared preference
  await platformFFI.init(appType);
  // global FFI, use this **ONLY** for global configuration
  // for convenience, use global FFI on mobile platform
  // focus on multi-ffi on desktop first
  await initGlobalFFI();
  // await Firebase.initializeApp();
  _registerEventHandler();
  // Update the system theme.
  updateSystemWindowTheme();
}

void runMainApp(bool startService) async {
  // register uni links
  await initEnv(kAppTypeMain);
  checkUpdate();
  // trigger connection status updater
  await bind.mainCheckConnectStatus();
  if (startService) {
    gFFI.serverModel.startService();
  }
  await Future.wait([gFFI.abModel.loadCache(), gFFI.groupModel.loadCache()]);
  gFFI.userModel.refreshCurrentUser();
  runApp(App());

  bool? alwaysOnTop;
  if (isDesktop) {
    alwaysOnTop =
        bind.mainGetBuildinOption(key: "main-window-always-on-top") == 'Y';
  }

  // Set window option.
  WindowOptions windowOptions = getHiddenTitleBarWindowOptions(
      isMainWindow: true, alwaysOnTop: alwaysOnTop);
  windowManager.waitUntilReadyToShow(windowOptions, () async {
    // Restore the location of the main window before window hide or show.
    await restoreWindowPosition(WindowType.Main);
    // Check the startup argument, if we successfully handle the argument, we keep the main window hidden.
    final handledByUniLinks = await initUniLinks();
    debugPrint("handled by uni links: $handledByUniLinks");
    if (handledByUniLinks || handleUriLink(cmdArgs: kBootArgs)) {
      windowManager.hide();
    } else {
      windowManager.show();
      windowManager.focus();
      // Move registration of active main window here to prevent from async visible check.
      rustDeskWinManager.registerActiveWindow(kWindowMainId);
    }
    windowManager.setOpacity(1);
    windowManager.setTitle(isWindows ? 'OpenUU' : getWindowName());
    // Do not use `windowManager.setResizable()` here.
    setResizable(!bind.isIncomingOnly());
  });
}

void runMobileApp() async {
  await initEnv(kAppTypeMain);
  checkUpdate();
  if (isAndroid) androidChannelInit();
  if (isAndroid) platformFFI.syncAndroidServiceAppDirConfigPath();
  draggablePositions.load();
  await Future.wait([gFFI.abModel.loadCache(), gFFI.groupModel.loadCache()]);
  gFFI.userModel.refreshCurrentUser();
  runApp(App());
  await initUniLinks();
}

class App extends StatefulWidget {
  @override
  State<App> createState() => _AppState();
}

class _AppState extends State<App> with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.window.onPlatformBrightnessChanged = () {
      final userPreference = MyTheme.getThemeModePreference();
      if (userPreference != ThemeMode.system) return;
      WidgetsBinding.instance.handlePlatformBrightnessChanged();
      final systemIsDark =
          WidgetsBinding.instance.platformDispatcher.platformBrightness ==
              Brightness.dark;
      final ThemeMode to;
      if (systemIsDark) {
        to = ThemeMode.dark;
      } else {
        to = ThemeMode.light;
      }
      Get.changeThemeMode(to);
      // Synchronize the window theme of the system.
      updateSystemWindowTheme();
      if (desktopType == DesktopType.main) {
        bind.mainChangeTheme(dark: to.toShortString());
      }
    };
    WidgetsBinding.instance.addObserver(this);
    WidgetsBinding.instance.addPostFrameCallback((_) => _updateOrientation());
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeMetrics() {
    _updateOrientation();
  }

  void _updateOrientation() {
    if (isDesktop) return;

    // Don't use `MediaQuery.of(context).orientation` in `didChangeMetrics()`,
    // my test (Flutter 3.19.6, Android 14) is always the reverse value.
    // https://github.com/flutter/flutter/issues/60899
    // stateGlobal.isPortrait.value =
    //     MediaQuery.of(context).orientation == Orientation.portrait;

    final orientation = View.of(context).physicalSize.aspectRatio > 1
        ? Orientation.landscape
        : Orientation.portrait;
    stateGlobal.isPortrait.value = orientation == Orientation.portrait;
  }

  @override
  Widget build(BuildContext context) {
    // final analytics = FirebaseAnalytics.instance;
    final botToastBuilder = BotToastInit();
    return RefreshWrapper(builder: (context) {
      return MultiProvider(
        providers: [
          // global configuration
          // use session related FFI when in remote control or file transfer page
          ChangeNotifierProvider.value(value: gFFI.ffiModel),
          ChangeNotifierProvider.value(value: gFFI.imageModel),
          ChangeNotifierProvider.value(value: gFFI.cursorModel),
          ChangeNotifierProvider.value(value: gFFI.canvasModel),
          ChangeNotifierProvider.value(value: gFFI.peerTabModel),
        ],
        child: GetMaterialApp(
          navigatorKey: globalKey,
          debugShowCheckedModeBanner: false,
          title: isWeb
              ? '${bind.mainGetAppNameSync()} Web Client V2 (Preview)'
              : bind.mainGetAppNameSync(),
          theme: MyTheme.lightTheme,
          darkTheme: MyTheme.darkTheme,
          themeMode: MyTheme.currentThemeMode(),
          home: isDesktop
              ? const DesktopTabPage()
              : isWeb
                  ? WebHomePage()
                  : HomePage(),
          localizationsDelegates: const [
            GlobalMaterialLocalizations.delegate,
            GlobalWidgetsLocalizations.delegate,
            GlobalCupertinoLocalizations.delegate,
          ],
          supportedLocales: supportedLocales,
          navigatorObservers: [
            // FirebaseAnalyticsObserver(analytics: analytics),
            BotToastNavigatorObserver(),
          ],
          builder: isAndroid
              ? (context, child) => AccessibilityListener(
                    child: MediaQuery(
                      data: MediaQuery.of(context).copyWith(
                        textScaler: TextScaler.linear(1.0),
                      ),
                      child: child ?? Container(),
                    ),
                  )
              : (context, child) {
                  child = _keepScaleBuilder(context, child);
                  child = botToastBuilder(context, child);
                  if ((isDesktop && desktopType == DesktopType.main) ||
                      isWebDesktop) {
                    child = keyListenerBuilder(context, child);
                  }
                  if (isLinux) {
                    return buildVirtualWindowFrame(context, child);
                  } else {
                    return workaroundWindowBorder(context, child);
                  }
                },
        ),
      );
    });
  }
}

Widget _keepScaleBuilder(BuildContext context, Widget? child) {
  return MediaQuery(
    data: MediaQuery.of(context).copyWith(
      textScaler: TextScaler.linear(1.0),
    ),
    child: child ?? Container(),
  );
}

_registerEventHandler() {
  if (isDesktop && desktopType != DesktopType.main) {
    platformFFI.registerEventHandler('theme', 'theme', (evt) async {
      String? dark = evt['dark'];
      if (dark != null) {
        await MyTheme.changeDarkMode(MyTheme.themeModeFromString(dark));
      }
    });
    platformFFI.registerEventHandler('language', 'language', (_) async {
      reloadAllWindows();
    });
  }
  if (isAndroid) {
    platformFFI.registerEventHandler(
        'android_needs_deploy', 'android_needs_deploy', (_) async {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        showDeployPromptDialog();
      });
    });
  }
}

Widget keyListenerBuilder(BuildContext context, Widget? child) {
  return RawKeyboardListener(
    // `skipTraversal: isWeb` is to fix "Bad state: RenderBox was not laid out: minified:aeL#c19e4"
    focusNode: FocusNode(skipTraversal: isWeb),
    child: child ?? Container(),
    onKey: (RawKeyEvent event) {
      if (event.logicalKey == LogicalKeyboardKey.shiftLeft) {
        if (event is RawKeyDownEvent) {
          gFFI.peerTabModel.setShiftDown(true);
        } else if (event is RawKeyUpEvent) {
          gFFI.peerTabModel.setShiftDown(false);
        }
      }
    },
  );
}
