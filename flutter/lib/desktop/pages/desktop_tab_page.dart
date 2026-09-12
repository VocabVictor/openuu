import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_home_page.dart';
import 'package:flutter_hbb/desktop/pages/desktop_setting_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';
// import 'package:flutter/services.dart';

import '../../common/shared_state.dart';
import '../../common/widgets/login.dart';
import 'desktop_welcome_page.dart';

class DesktopTabPage extends StatefulWidget {
  const DesktopTabPage({Key? key}) : super(key: key);

  @override
  State<DesktopTabPage> createState() => _DesktopTabPageState();

  static void showHome({bool assistance = false}) {
    final page = Get.find<_DesktopTabPageState>();
    page.showHome(assistance);
  }

  static void onAddSetting(
      {SettingsTabKey initialPage = SettingsTabKey.general}) {
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
  }
}

class _DesktopTabPageState extends State<DesktopTabPage> {
  final tabController = DesktopTabController(tabType: DesktopTabType.main);
  bool _showAssistance = false;

  void showHome(bool assistance) {
    setState(() => _showAssistance = assistance);
    tabController.jumpTo(0);
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
              tail: Offstage(
                offstage: bind.isIncomingOnly() || bind.isDisableSettings(),
                child: ActionIcon(
                  message: 'Settings',
                  icon: IconFont.menu,
                  onTap: DesktopTabPage.onAddSetting,
                  isClose: false,
                ),
              ),
            )));
    final content = Obx(() {
      final signedIn = gFFI.userModel.isLogin;
      final homeSelected =
          tabController.state.value.selectedTabInfo.key == kTabLabelHomePage;
      final showWelcome = isWindows &&
          !bind.isIncomingOnly() &&
          !signedIn &&
          homeSelected &&
          !_showAssistance;
      return Stack(children: [
        // Keep the existing home mounted: it owns remote-window event handlers.
        Offstage(
            offstage: showWelcome,
            child: Column(children: [
              if (isWindows && !signedIn && _showAssistance && homeSelected)
                Material(
                    child: Align(
                        alignment: Alignment.centerLeft,
                        child: TextButton.icon(
                            onPressed: () =>
                                setState(() => _showAssistance = false),
                            icon: const Icon(Icons.arrow_back),
                            label: const Text('OpenUU')))),
              Expanded(child: tabWidget),
            ])),
        if (showWelcome)
          DesktopWelcomePage(
            onLogin: () {
              loginDialog();
            },
            onAssistance: () => setState(() => _showAssistance = true),
            onFavorites: () {
              gFFI.peerTabModel.setCurrentTab(1);
              setState(() => _showAssistance = true);
            },
            onSettings: bind.isDisableSettings()
                ? null
                : () => DesktopTabPage.onAddSetting(),
          ),
      ]);
    });
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
