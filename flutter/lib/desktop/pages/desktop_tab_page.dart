import '../../models/wol_model.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/account_action.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
// import 'package:flutter/services.dart';

import '../../common/shared_state.dart';
import '../../common/widgets/login.dart';
import 'desktop_welcome_page.dart';
import 'desktop_devices_page.dart';
import 'desktop_favorites_page.dart';

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
    if (bind.isIncomingOnly()) {
      // The incoming-only build has no home shell; keep the settings tab.
      try {
        DesktopTabController tabController = Get.find<DesktopTabController>();
        tabController.add(TabInfo(
            key: kTabLabelSettingPage,
            label: kTabLabelSettingPage,
            selectedIcon: Icons.build_sharp,
            unselectedIcon: Icons.build_outlined,
            page: DesktopSettingPage(
              key: const ValueKey(kTabLabelSettingPage),
              initialTabkey: initialPage,
            )));
      } catch (e) {
        debugPrintStack(label: '$e');
      }
      return;
    }
    final page = Get.find<_DesktopTabPageState>();
    page.showSettings(initialPage);
  }
}

class _DesktopTabPageState extends State<DesktopTabPage> {
  final tabController = DesktopTabController(tabType: DesktopTabType.main);
  bool _showAssistance = false;
  bool _showFavorites = false;
  bool _showSettings = false;
  SettingsTabKey _settingsTab = SettingsTabKey.general;
  final _wol = WolModel();

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
        page: DesktopHomePage(
          key: const ValueKey(kTabLabelHomePage),
        )));
    if (bind.isIncomingOnly()) {
      tabController.onSelected = (key) {
        if (key == kTabLabelHomePage) {
          windowManager.setSize(getIncomingOnlyHomeSize());
          setResizable(false);
        } else {
          windowManager.setSize(getIncomingOnlySettingsSize());
          setResizable(true);
        }
      };
    }
  }

  @override
  void initState() {
    super.initState();
    if (isWindows) _wol.start();
    // HardwareKeyboard.instance.addHandler(_handleKeyEvent);
  }

  /*
  bool _handleKeyEvent(KeyEvent event) {
    if (!mouseIn && event is KeyDownEvent) {
      print('key down: ${event.logicalKey}');
      shouldBeBlocked(_block, canBeBlocked);
    }
    return false; // allow it to propagate
  }
  */

  @override
  void dispose() {
    _wol.dispose();
    // HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    Get.delete<DesktopTabController>();
    Get.delete<_DesktopTabPageState>();

    super.dispose();
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
                final showSettings = isWindows &&
                    !bind.isIncomingOnly() &&
                    homeSelected &&
                    _showSettings;
                final showWelcome = isWindows &&
                    !bind.isIncomingOnly() &&
                    !signedIn &&
                    homeSelected &&
                    !_showSettings;
                final showFavorites = isWindows &&
                    !bind.isIncomingOnly() &&
                    signedIn &&
                    homeSelected &&
                    _showFavorites &&
                    !_showSettings;
                final showDevices = isWindows &&
                    !bind.isIncomingOnly() &&
                    signedIn &&
                    homeSelected &&
                    !_showAssistance &&
                    !_showFavorites &&
                    !_showSettings;
                return Stack(children: [
                  // Keep the existing home mounted: it owns remote-window event handlers.
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
              tail: Row(mainAxisSize: MainAxisSize.min, children: [
                if (isWindows && !bind.isIncomingOnly())
                  const AccountAction(),
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
