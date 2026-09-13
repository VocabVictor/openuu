import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_assistance_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/account_action.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/utils/platform_channel.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
import 'package:window_size/window_size.dart' as window_size;

import '../../../common/shared_state.dart';
import '../../../common/widgets/login.dart';
import '../../../models/wol_model.dart';
import '../desktop_welcome_page.dart';
import '../desktop_devices_page.dart';
import '../desktop_favorites_page.dart';

part 'assistance_home.dart';
part 'window_handlers.dart';

class DesktopTabPage extends StatefulWidget {
  const DesktopTabPage({Key? key}) : super(key: key);

  @override
  State<DesktopTabPage> createState() => _DesktopTabPageState();

  static void showHome({bool assistance = false, bool favorites = false}) {
    if ((assistance || favorites) && !gFFI.userModel.isLogin) {
      loginDialog();
      return;
    }
    final page = Get.find<_DesktopTabPageState>();
    page.showHome(assistance, favorites);
  }

  /// Show the settings inside the home shell, on `initialPage`.
  static void onAddSetting(
      {SettingsTabKey initialPage = SettingsTabKey.general}) {
    final page = Get.find<_DesktopTabPageState>();
    page.showSettings(initialPage);
  }
}

class _DesktopTabPageState extends State<DesktopTabPage>
    with WidgetsBindingObserver {
  final tabController = DesktopTabController(tabType: DesktopTabType.main);
  bool _showAssistance = false;
  bool _showFavorites = false;
  bool _showSettings = false;
  SettingsTabKey _settingsTab = SettingsTabKey.general;
  final _wol = WolModel();
  // Main-window state the assistance home and the window handlers share.
  final svcStopped = false.obs;
  final RxBool _block = false.obs;
  StreamSubscription? _uniLinksSubscription;
  Timer? _updateTimer;

  void _setState(VoidCallback fn) => setState(fn);

  void showHome(bool assistance, bool favorites) {
    setState(() {
      _showAssistance = assistance;
      _showFavorites = favorites;
      _showSettings = false;
    });
    tabController.jumpTo(0);
  }

  void showSettings(SettingsTabKey tab) {
    setState(() {
      _showSettings = true;
      _settingsTab = tab;
    });
    tabController.jumpTo(0);
    // The page keeps one instance; once it is mounted, jump to the requested
    // sub-page instead of rebuilding it.
    WidgetsBinding.instance
        .addPostFrameCallback((_) => DesktopSettingPage.jumpToMounted(tab));
  }

  _DesktopTabPageState() {
    Get.put<_DesktopTabPageState>(this);
    RemoteCountState.init();
    Get.put<DesktopTabController>(tabController);
    tabController.add(TabInfo(
        key: kTabLabelHomePage,
        label: kTabLabelHomePage,
        selectedIcon: Icons.home_sharp,
        unselectedIcon: Icons.home_outlined,
        closable: false,
        page: _KeepAlive(
            key: const ValueKey(kTabLabelHomePage),
            child: Builder(builder: (context) => _assistanceHome(context)))));
  }

  @override
  void initState() {
    super.initState();
    if (isWindows) _wol.start();
    _initWindowHandlers();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _disposeWindowHandlers();
    _wol.dispose();
    Get.delete<DesktopTabController>();
    Get.delete<_DesktopTabPageState>();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.resumed) {
      shouldBeBlocked(_block, canBeBlocked);
    }
  }

  @override
  Widget build(BuildContext context) {
    final tabWidget = Container(
        child: Scaffold(
            backgroundColor: Theme.of(context).colorScheme.background,
            body: DesktopTab(
              controller: tabController,
              pageViewBuilder: (pageView) => Obx(() {
                final signedIn = gFFI.userModel.isLogin;
                final homeSelected =
                    tabController.state.value.selectedTabInfo.key ==
                        kTabLabelHomePage;
                final showSettings = homeSelected && _showSettings;
                final showWelcome =
                    !signedIn && homeSelected && !_showSettings;
                final showFavorites = signedIn &&
                    homeSelected &&
                    _showFavorites &&
                    !_showSettings;
                final showDevices = signedIn &&
                    homeSelected &&
                    !_showAssistance &&
                    !_showFavorites &&
                    !_showSettings;
                return Stack(children: [
                  // Keep the home tab mounted: the assistance page lives there.
                  Offstage(
                      offstage: showWelcome ||
                          showDevices ||
                          showFavorites ||
                          showSettings,
                      child: pageView),
                  if (showDevices) const DesktopDevicesPage(),
                  if (showFavorites) const DesktopFavoritesPage(),
                  if (showSettings)
                    DesktopSettingPage(
                        key: const ValueKey(kTabLabelSettingPage),
                        initialTabkey: _settingsTab),
                  if (showWelcome)
                    DesktopWelcomePage(
                      onLogin: () {
                        loginDialog();
                      },
                      onAssistance: () => loginDialog(),
                      onFavorites: () =>
                          DesktopTabPage.showHome(favorites: true),
                      onSettings: bind.isDisableSettings()
                          ? null
                          : () => DesktopTabPage.onAddSetting(),
                    ),
                ]);
              }),
              tail: const Row(mainAxisSize: MainAxisSize.min, children: [
                AccountAction(),
              ]),
            )));
    final content = tabWidget;
    return isMacOS || kUseCompatibleUiMode
        ? content
        : Obx(
            () => DragToResizeArea(
              resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
              enableResizeEdges: windowManagerEnableResizeEdges,
              child: content,
            ),
          );
  }
}
