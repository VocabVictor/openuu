import 'package:flutter_hbb/common/widgets/brand_icon.dart';
import '../desktop_welcome_page.dart';
import '../../widgets/settings_dropdown.dart';
import '../../widgets/settings_panel.dart';
import '../../widgets/settings_row.dart';
import '../../widgets/ui_tokens.dart';
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/audio_input.dart';
import 'package:flutter_hbb/common/widgets/custom_password.dart';
import 'package:flutter_hbb/common/widgets/setting_widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/pages/desktop_tab_page.dart';
import 'package:flutter_hbb/desktop/widgets/pinned_session_setting.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/desktop/widgets/update_progress.dart';
import 'package:flutter_hbb/mobile/widgets/dialog.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/server_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:password_strength/password_strength.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../../../common/widgets/dialog.dart';
import '../../../common/widgets/login.dart';

part 'about_desktop.dart';
part 'account_about.dart';
part 'constants.dart';
part 'controls.dart';
part 'desktop_rows.dart';
part 'display.dart';
part 'display_quality.dart';
part 'display_style.dart';
part 'general.dart';
part 'general_desktop.dart';
part 'general_other.dart';
part 'general_record.dart';
part 'network.dart';
part 'network_desktop.dart';
part 'network_proxy_panel.dart';
part 'network_server_panel.dart';
part 'page_tabs.dart';
part 'safety.dart';
part 'safety_network.dart';
part 'safety_password.dart';
part 'set_password_dialog.dart';
part 'set_password_dialog_desktop.dart';
part 'safety_permissions.dart';
part 'safety_tfa.dart';
part 'socks5.dart';
part 'wayland.dart';
part 'widgets.dart';

class DesktopSettingPage extends StatefulWidget {
  final SettingsTabKey initialTabkey;
  static final List<SettingsTabKey> tabKeys = [
    if (bind.mainGetBuildinOption(key: kOptionHideGeneralSetting) != 'Y')
      SettingsTabKey.general,
    if (!bind.isOutgoingOnly() &&
        !bind.isDisableSettings() &&
        bind.mainGetBuildinOption(key: kOptionHideSecuritySetting) != 'Y')
      SettingsTabKey.safety,
    if (!bind.isDisableSettings() &&
        bind.mainGetBuildinOption(key: kOptionHideNetworkSetting) != 'Y')
      SettingsTabKey.network,
    if (!bind.isIncomingOnly()) SettingsTabKey.display,
    SettingsTabKey.about,
  ];

  DesktopSettingPage({Key? key, required this.initialTabkey}) : super(key: key);

  @override
  State<DesktopSettingPage> createState() =>
      _DesktopSettingPageState(initialTabkey);

  static void switch2page(SettingsTabKey page) {
    if (!tabKeys.contains(page)) {
      return;
    }
    DesktopTabPage.onAddSetting(initialPage: page);
  }

  /// Jump the mounted settings page to `page`; a no-op before it is mounted,
  /// when the constructor's initialTabkey applies instead.
  static void jumpToMounted(SettingsTabKey page) {
    try {
      int index = tabKeys.indexOf(page);
      if (index == -1) {
        return;
      }
      if (Get.isRegistered<PageController>(tag: _kSettingPageControllerTag) &&
          Get.isRegistered<Rx<SettingsTabKey>>(tag: _kSettingPageTabKeyTag)) {
        PageController controller =
            Get.find<PageController>(tag: _kSettingPageControllerTag);
        Rx<SettingsTabKey> selected =
            Get.find<Rx<SettingsTabKey>>(tag: _kSettingPageTabKeyTag);
        selected.value = page;
        if (controller.hasClients) {
          controller.jumpToPage(index);
        }
      }
    } catch (e) {
      debugPrintStack(label: '$e');
    }
  }
}

class _DesktopSettingPageState extends State<DesktopSettingPage>
    with
        TickerProviderStateMixin,
        AutomaticKeepAliveClientMixin,
        WidgetsBindingObserver {
  late PageController controller;
  late Rx<SettingsTabKey> selectedTab;

  @override
  bool get wantKeepAlive => true;

  final RxBool _block = false.obs;
  final RxBool _canBeBlocked = false.obs;
  Timer? _videoConnTimer;

  _DesktopSettingPageState(SettingsTabKey initialTabkey) {
    var initialIndex = DesktopSettingPage.tabKeys.indexOf(initialTabkey);
    if (initialIndex == -1) {
      initialIndex = 0;
    }
    selectedTab = DesktopSettingPage.tabKeys[initialIndex].obs;
    Get.put<Rx<SettingsTabKey>>(selectedTab, tag: _kSettingPageTabKeyTag);
    controller = PageController(initialPage: initialIndex);
    Get.put<PageController>(controller, tag: _kSettingPageControllerTag);
    controller.addListener(() {
      if (controller.page != null) {
        int page = controller.page!.toInt();
        if (page < DesktopSettingPage.tabKeys.length) {
          selectedTab.value = DesktopSettingPage.tabKeys[page];
        }
      }
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.resumed) {
      shouldBeBlocked(_block, canBeBlocked);
    }
  }

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _videoConnTimer =
        periodic_immediate(Duration(milliseconds: 1000), () async {
      if (!mounted) {
        return;
      }
      final blocked = await canBeBlocked();
      if (!mounted) {
        return;
      }
      _canBeBlocked.value = blocked;
    });
  }

  @override
  void dispose() {
    _videoConnTimer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    Get.delete<PageController>(tag: _kSettingPageControllerTag);
    Get.delete<Rx<SettingsTabKey>>(tag: _kSettingPageTabKeyTag);
    // Get.delete does not dispose a plain ChangeNotifier.
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    if (isWindows && !bind.isIncomingOnly()) {
      final theme = MyTheme.lightTheme;
      return Theme(
          data: theme.copyWith(
            scaffoldBackgroundColor: const Color(0xfff8fbfd),
            textTheme: theme.textTheme.apply(fontFamily: 'Microsoft YaHei'),
            cardTheme: const CardTheme(
                color: Colors.white,
                surfaceTintColor: Colors.transparent,
                elevation: 0,
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.all(Radius.circular(5)),
                    side: BorderSide(color: Color(0xffdfe4e8)))),
          ),
          child: DesktopWelcomePage(
            settingsSelected: true,
            onLogin: () {
              loginDialog();
            },
            onDevices: () => DesktopTabPage.showHome(),
            onAssistance: () => DesktopTabPage.showHome(assistance: true),
            onFavorites: () => DesktopTabPage.showHome(favorites: true),
            onSettings: () {},
            content: _buildBlock(children: [
              Expanded(
                  child: Column(children: [
                Padding(
                    padding: const EdgeInsets.symmetric(
                        horizontal: UiSpace.pagePaddingX),
                    child: SizedBox(
                        height: UiSpace.settingsTabBarHeight,
                        width: double.infinity,
                        child: SingleChildScrollView(
                            scrollDirection: Axis.horizontal,
                            child: Row(children: [
                              for (final tab in _settingTabs())
                                Obx(() => _tabLabel(tab,
                                    selectedTab.value == tab.key, () {
                                      controller.jumpToPage(DesktopSettingPage
                                          .tabKeys
                                          .indexOf(tab.key));
                                      selectedTab.value = tab.key;
                                    })),
                            ])))),
                const SizedBox(height: UiSpace.s4),
                Expanded(
                    child: Align(
                        alignment: Alignment.topLeft,
                        child: ConstrainedBox(
                            constraints: const BoxConstraints(
                                maxWidth: UiSpace.settingsContentMaxWidth +
                                    2 * UiSpace.pagePaddingX),
                            child: Padding(
                                padding: const EdgeInsets.symmetric(
                                    horizontal: UiSpace.pagePaddingX),
                                child: PageView(
                                    controller: controller,
                                    physics:
                                        const NeverScrollableScrollPhysics(),
                                    children: _children()))))),
              ]))
            ]),
          ));
    }
    return Scaffold(
      backgroundColor: Theme.of(context).colorScheme.background,
      body: _buildBlock(
        children: <Widget>[
          SizedBox(
            width: _kTabWidth,
            child: Column(
              children: [
                _header(context),
                Flexible(child: _listView(tabs: _settingTabs())),
              ],
            ),
          ),
          const VerticalDivider(width: 1),
          Expanded(
            child: Container(
              color: Theme.of(context).scaffoldBackgroundColor,
              child: PageView(
                controller: controller,
                physics: NeverScrollableScrollPhysics(),
                children: _children(),
              ),
            ),
          )
        ],
      ),
    );
  }

}
