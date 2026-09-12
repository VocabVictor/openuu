import '../../widgets/quick_launch.dart';
import 'dart:convert';
import 'dart:async';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/remote_page.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';
import 'package:bot_toast/bot_toast.dart';

import '../../../common/widgets/dialog.dart';
import '../../../models/platform_model.dart';
import 'package:flutter_hbb/models/model.dart';
part 'method_handler.dart';
part 'tab_menu.dart';

class _MenuTheme {
  static const Color blueColor = MyTheme.button;
  // kMinInteractiveDimension
  static const double height = 20.0;
  static const double dividerHeight = 12.0;
}

class ConnectionTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const ConnectionTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<ConnectionTabPage> createState() => _ConnectionTabPageState(params);
}

class _ConnectionTabPageState extends State<ConnectionTabPage> {
  final tabController =
      Get.put(DesktopTabController(tabType: DesktopTabType.remoteScreen));
  final contentKey = UniqueKey();
  static const IconData selectedIcon = Icons.desktop_windows_sharp;
  static const IconData unselectedIcon = Icons.desktop_windows_outlined;

  String? peerId;
  bool _isScreenRectSet = false;
  int? _display;

  var connectionMap = RxList<Widget>.empty(growable: true);

  _ConnectionTabPageState(Map<String, dynamic> params) {
    RemoteCountState.init();
    peerId = params['id'];
    final sessionId = params['session_id'];
    final tabWindowId = params['tab_window_id'];
    final display = params['display'];
    final displays = params['displays'];
    final screenRect = parseParamScreenRect(params);
    _isScreenRectSet = screenRect != null;
    _display = display as int?;
    tryMoveToScreenAndSetFullscreen(screenRect);
    if (peerId != null) {
      ConnectionTypeState.init(peerId!);
      tabController.onSelected = (id) {
        final remotePage = tabController.widget(id);
        if (remotePage is RemotePage) {
          final ffi = remotePage.ffi;
          bind.setCurSessionId(sessionId: ffi.sessionId);
        }
        WindowController.fromWindowId(params['windowId'])
            .setTitle(getWindowNameWithId(id));
        UnreadChatCountState.find(id).value = 0;
      };
      tabController.add(TabInfo(
        key: peerId!,
        label: peerId!,
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        onTabCloseButton: () async {
          if (await desktopTryShowTabAuditDialogCloseCancelled(
            id: peerId!,
            tabController: tabController,
          )) {
            return;
          }
          tabController.closeBy(peerId!);
        },
        page: RemotePage(
          key: ValueKey(peerId),
          id: peerId!,
          sessionId: sessionId == null ? null : SessionID(sessionId),
          tabWindowId: tabWindowId,
          display: display,
          displays: displays?.cast<int>(),
          password: params['password'],
          toolbarState: ToolbarState(),
          tabController: tabController,
          switchUuid: params['switch_uuid'],
          quickLaunch: params['quickLaunch'],
          viewOnly: params['viewOnly'] == true,
          forceRelay: params['forceRelay'],
          isSharedPassword: params['isSharedPassword'],
        ),
      ));
      _update_remote_count();
    }
    tabController.onRemoved = (_, id) => onRemoveId(id);
    rustDeskWinManager.setMethodHandler(_remoteMethodHandler);
  }

  @override
  void initState() {
    super.initState();

    if (!_isScreenRectSet) {
      Future.delayed(Duration.zero, () {
        restoreWindowPosition(
          WindowType.RemoteDesktop,
          windowId: windowId(),
          peerId: tabController.state.value.tabs.isEmpty
              ? null
              : tabController.state.value.tabs[0].key,
          display: _display,
        );
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final child = Scaffold(
      backgroundColor: Theme.of(context).colorScheme.background,
      body: DesktopTab(
        controller: tabController,
        onWindowCloseButton: handleWindowCloseButton,
        tail: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _RelativeMouseModeHint(tabController: tabController),
            const AddButton(),
          ],
        ),
        selectedBorderColor: MyTheme.accent,
        pageViewBuilder: (pageView) => pageView,
        labelGetter: DesktopTab.tablabelGetter,
        tabBuilder: (key, icon, label, themeConf) => Obx(() {
          final connectionType = ConnectionTypeState.find(key);
          if (!connectionType.isValid()) {
            return Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                icon,
                label,
              ],
            );
          } else {
            bool secure =
                connectionType.secure.value == ConnectionType.strSecure;
            bool direct =
                connectionType.direct.value == ConnectionType.strDirect;
            String msgConn = getConnectionText(
                secure, direct, connectionType.stream_type.value);
            var msgFingerprint = '${translate('Fingerprint')}:\n';
            var fingerprint = FingerprintState.find(key).value;
            if (fingerprint.isEmpty) {
              fingerprint = 'N/A';
            }
            if (fingerprint.length > 5 * 8) {
              var first = fingerprint.substring(0, 39);
              var second = fingerprint.substring(40);
              msgFingerprint += '$first\n$second';
            } else {
              msgFingerprint += fingerprint;
            }

            final tab = Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                icon,
                Tooltip(
                  message: '$msgConn\n$msgFingerprint',
                  child: SvgPicture.asset(
                    'assets/${connectionType.secure.value}${connectionType.direct.value}.svg',
                    width: themeConf.iconSize,
                    height: themeConf.iconSize,
                  ).paddingOnly(right: 5),
                ),
                label,
                unreadMessageCountBuilder(UnreadChatCountState.find(key))
                    .marginOnly(left: 4),
              ],
            );

            return Listener(
              onPointerDown: (e) {
                if (e.kind != ui.PointerDeviceKind.mouse) {
                  return;
                }
                final remotePage = tabController.state.value.tabs
                    .firstWhere((tab) => tab.key == key)
                    .page as RemotePage;
                if (remotePage.ffi.ffiModel.pi.isSet.isTrue && e.buttons == 2) {
                  showRightMenu(
                    (CancelFunc cancelFunc) {
                      return _tabMenuBuilder(key, cancelFunc);
                    },
                    target: e.position,
                  );
                }
              },
              child: tab,
            );
          }
        }),
      ),
    );
    final tabWidget = isLinux
        ? buildVirtualWindowFrame(context, child)
        : workaroundWindowBorder(
            context,
            Obx(() => Container(
                  decoration: BoxDecoration(
                    border: Border.all(
                        color: MyTheme.color(context).border!,
                        width: stateGlobal.windowBorderWidth.value),
                  ),
                  child: child,
                )));
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : Obx(() => SubWindowDragToResizeArea(
              key: contentKey,
              child: tabWidget,
              // Specially configured for a better resize area and remote control.
              childPadding: kDragToResizeAreaPadding,
              resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
              enableResizeEdges: subWindowManagerEnableResizeEdges,
              windowId: stateGlobal.windowId,
            ));
  }

  _update_remote_count() =>
      RemoteCountState.find().value = tabController.length;

}
