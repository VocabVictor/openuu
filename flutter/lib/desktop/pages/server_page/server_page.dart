// original cm window in Sciter version.

import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/audio_input.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/cm_file_model.dart';
import 'package:flutter_hbb/utils/platform_channel.dart';
import 'package:get/get.dart';
import 'package:percent_indicator/linear_percent_indicator.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../../common.dart';
import '../../../common/widgets/chat_page.dart';
import '../../../models/file_model.dart';
import '../../../models/platform_model.dart';
import '../../../models/server_model.dart';
import 'package:flutter_hbb/models/model.dart';
part 'cm_control_panel_authorized.dart';
part 'cm_control_panel.dart';
part 'cm_header.dart';
part 'privilege_board.dart';

/// Set only by this window's own close control, and only once the user has confirmed. Any other
/// way the window can go - a session logout closing every window, the window manager, a native
/// title-bar button this app does not draw - leaves it false, which is the honest answer:
/// nothing in that close says who asked for it. It lives at file scope because the control that
/// sets it (`ConnectionManagerState`) and the handler that reads it (`_DesktopServerPageState`)
/// are different widgets.
bool _cmClosedByOperator = false;

class DesktopServerPage extends StatefulWidget {
  const DesktopServerPage({Key? key}) : super(key: key);

  @override
  State<DesktopServerPage> createState() => _DesktopServerPageState();
}

class _DesktopServerPageState extends State<DesktopServerPage>
    with WindowListener, AutomaticKeepAliveClientMixin {
  final tabController = gFFI.serverModel.tabController;

  _DesktopServerPageState() {
    gFFI.ffiModel.updateEventListener(gFFI.sessionId, "");
    Get.put<DesktopTabController>(tabController);
    tabController.onRemoved = (_, id) {
      onRemoveId(id);
    };
  }

  @override
  void initState() {
    windowManager.addListener(this);
    super.initState();
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowClose() {
    // Other platforms keep the old behaviour exactly: the ambiguity this guards against is a
    // Linux session logout, which closes every window in the session.
    final byOperator = _cmClosedByOperator || !isLinux;
    Future.wait([gFFI.serverModel.closeAll(byOperator: byOperator), gFFI.close()]).then((_) {
      if (isMacOS) {
        RdPlatformChannel.instance.terminate();
      } else {
        windowManager.setPreventClose(false);
        windowManager.close();
      }
    });
    super.onWindowClose();
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      windowManager.close();
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: gFFI.serverModel),
        ChangeNotifierProvider.value(value: gFFI.chatModel),
      ],
      child: Consumer<ServerModel>(
        builder: (context, serverModel, child) {
          final body = Scaffold(
            backgroundColor: Theme.of(context).colorScheme.background,
            body: ConnectionManager(),
          );
          return isLinux
              ? buildVirtualWindowFrame(context, body)
              : workaroundWindowBorder(
                  context,
                  Container(
                    decoration: BoxDecoration(
                        border:
                            Border.all(color: MyTheme.color(context).border!)),
                    child: body,
                  ));
        },
      ),
    );
  }

  @override
  bool get wantKeepAlive => true;
}

class ConnectionManager extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => ConnectionManagerState();
}

class ConnectionManagerState extends State<ConnectionManager>
    with WidgetsBindingObserver {
  final RxBool _controlPageBlock = false.obs;
  final RxBool _sidePageBlock = false.obs;

  ConnectionManagerState() {
    gFFI.serverModel.tabController.onSelected = (client_id_str) {
      final client_id = int.tryParse(client_id_str);
      if (client_id != null) {
        final client =
            gFFI.serverModel.clients.firstWhereOrNull((e) => e.id == client_id);
        if (client != null) {
          gFFI.chatModel.changeCurrentKey(MessageKey(client.peerId, client.id));
          if (client.unreadChatMessageCount.value > 0) {
            WidgetsBinding.instance.addPostFrameCallback((_) {
              client.unreadChatMessageCount.value = 0;
              gFFI.chatModel.showChatPage(MessageKey(client.peerId, client.id));
            });
          }
          windowManager.setTitle(getWindowNameWithId(client.peerId));
          gFFI.cmFileModel.updateCurrentClientId(client.id);
        }
      }
    };
    gFFI.chatModel.isConnManager = true;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.resumed) {
      if (!allowRemoteCMModification()) {
        shouldBeBlocked(_controlPageBlock, null);
        shouldBeBlocked(_sidePageBlock, null);
      }
    }
  }

  @override
  void initState() {
    gFFI.serverModel.updateClientState();
    WidgetsBinding.instance.addObserver(this);
    super.initState();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    pointerHandler(PointerEvent e) {
      if (serverModel.cmHiddenTimer != null) {
        serverModel.cmHiddenTimer!.cancel();
        serverModel.cmHiddenTimer = null;
        debugPrint("CM hidden timer has been canceled");
      }
    }

    return serverModel.clients.isEmpty
        ? Column(
            children: [
              buildTitleBar(),
              Expanded(
                child: Center(
                  child: Text(translate("Waiting")),
                ),
              ),
            ],
          )
        : Listener(
            onPointerDown: pointerHandler,
            onPointerMove: pointerHandler,
            child: DesktopTab(
              showTitle: false,
              showMaximize: false,
              showMinimize: true,
              showClose: true,
              onWindowCloseButton: handleWindowCloseButton,
              controller: serverModel.tabController,
              selectedBorderColor: MyTheme.accent,
              maxLabelWidth: 100,
              tail: null, //buildScrollJumper(),
              tabBuilder: (key, icon, label, themeConf) {
                final client = serverModel.clients
                    .firstWhereOrNull((client) => client.id.toString() == key);
                return Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Tooltip(
                        message: key,
                        waitDuration: Duration(seconds: 1),
                        child: label),
                    unreadMessageCountBuilder(client?.unreadChatMessageCount)
                        .marginOnly(left: 4),
                  ],
                );
              },
              pageViewBuilder: (pageView) => LayoutBuilder(
                builder: (context, constrains) {
                  var borderWidth = 0.0;
                  if (constrains.maxWidth >
                      kConnectionManagerWindowSizeClosedChat.width) {
                    borderWidth = kConnectionManagerWindowSizeOpenChat.width -
                        constrains.maxWidth;
                  } else {
                    borderWidth = kConnectionManagerWindowSizeClosedChat.width -
                        constrains.maxWidth;
                  }
                  if (borderWidth < 0 || borderWidth > 50) {
                    borderWidth = 0;
                  }
                  final realClosedWidth =
                      kConnectionManagerWindowSizeClosedChat.width -
                          borderWidth;
                  final realChatPageWidth =
                      constrains.maxWidth - realClosedWidth;
                  final row = Row(children: [
                    if (constrains.maxWidth >
                        kConnectionManagerWindowSizeClosedChat.width)
                      Consumer<ChatModel>(
                          builder: (_, model, child) => SizedBox(
                                width: realChatPageWidth,
                                child: allowRemoteCMModification()
                                    ? buildSidePage()
                                    : buildRemoteBlock(
                                        child: buildSidePage(),
                                        block: _sidePageBlock,
                                        mask: true),
                              )),
                    SizedBox(
                        width: realClosedWidth,
                        child: SizedBox(
                            width: realClosedWidth,
                            child: allowRemoteCMModification()
                                ? pageView
                                : buildRemoteBlock(
                                    child: _buildKeyEventBlock(pageView),
                                    block: _controlPageBlock,
                                    mask: false,
                                  ))),
                  ]);
                  return Container(
                    color: Theme.of(context).scaffoldBackgroundColor,
                    child: row,
                  );
                },
              ),
            ),
          );
  }

  Widget buildSidePage() {
    final selected = gFFI.serverModel.tabController.state.value.selected;
    if (selected < 0 || selected >= gFFI.serverModel.clients.length) {
      return Offstage();
    }
    final clientType = gFFI.serverModel.clients[selected].type_();
    if (clientType == ClientType.file) {
      return _FileTransferLogPage();
    } else {
      return ChatPage(type: ChatPageType.desktopCM);
    }
  }

  Widget _buildKeyEventBlock(Widget child) {
    return ExcludeFocus(child: child, excluding: true);
  }

  Widget buildTitleBar() {
    return SizedBox(
      height: kDesktopRemoteTabBarHeight,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const _AppIcon(),
          Expanded(
            child: GestureDetector(
              onPanStart: (d) {
                windowManager.startDragging();
              },
              child: Container(
                color: Theme.of(context).colorScheme.background,
              ),
            ),
          ),
          const SizedBox(
            width: 4.0,
          ),
          const _CloseButton()
        ],
      ),
    );
  }

  Widget buildScrollJumper() {
    final offstage = gFFI.serverModel.clients.length < 2;
    final sc = gFFI.serverModel.tabController.state.value.scrollController;
    return Offstage(
        offstage: offstage,
        child: Row(
          children: [
            ActionIcon(
                icon: Icons.arrow_left, iconSize: 22, onTap: sc.backward),
            ActionIcon(
                icon: Icons.arrow_right, iconSize: 22, onTap: sc.forward),
          ],
        ));
  }

  Future<bool> handleWindowCloseButton() async {
    var tabController = gFFI.serverModel.tabController;
    final connLength = tabController.length;
    if (connLength <= 1) {
      _cmClosedByOperator = true;
      windowManager.close();
      return true;
    } else {
      final bool res;
      if (!option2bool(kOptionEnableConfirmClosingTabs,
          bind.mainGetLocalOption(key: kOptionEnableConfirmClosingTabs))) {
        res = true;
      } else {
        res = await closeConfirmDialog();
      }
      if (res) {
        // After the dialog, never before it: an external close while it is open must not
        // inherit an intent the user had not expressed yet.
        _cmClosedByOperator = true;
        windowManager.close();
      }
      return res;
    }
  }
}

Widget buildConnectionCard(Client client) {
  return Consumer<ServerModel>(
    builder: (context, value, child) => Column(
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.start,
      key: ValueKey(client.id),
      children: [
        _CmHeader(client: client),
        client.type_() == ClientType.file ||
                client.type_() == ClientType.portForward ||
                client.type_() == ClientType.terminal ||
                client.disconnected
            ? Offstage()
            : _PrivilegeBoard(client: client),
        Expanded(
          child: Align(
            alignment: Alignment.bottomCenter,
            child: _CmControlPanel(client: client),
          ),
        )
      ],
    ).paddingSymmetric(vertical: 4.0, horizontal: 8.0),
  );
}

class _FileTransferLogPage extends StatefulWidget {
  _FileTransferLogPage({Key? key}) : super(key: key);

  @override
  State<_FileTransferLogPage> createState() => __FileTransferLogPageState();
}

class __FileTransferLogPageState extends State<_FileTransferLogPage> {
  @override
  Widget build(BuildContext context) {
    return statusList();
  }

  Widget generateCard(Widget child) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).cardColor,
        borderRadius: BorderRadius.all(
          Radius.circular(15.0),
        ),
      ),
      child: child,
    );
  }

  iconLabel(CmFileLog item) {
    switch (item.action) {
      case CmFileAction.none:
        return Container();
      case CmFileAction.localToRemote:
      case CmFileAction.remoteToLocal:
        return Column(
          children: [
            Transform.rotate(
              angle: item.action == CmFileAction.remoteToLocal ? 0 : pi,
              child: SvgPicture.asset(
                "assets/arrow.svg",
                colorFilter: svgColor(Theme.of(context).tabBarTheme.labelColor),
              ),
            ),
            Text(item.action == CmFileAction.remoteToLocal
                ? translate('Send')
                : translate('Receive'))
          ],
        );
      case CmFileAction.remove:
        return Column(
          children: [
            Icon(
              Icons.delete,
              color: Theme.of(context).tabBarTheme.labelColor,
            ),
            Text(translate('Delete'))
          ],
        );
      case CmFileAction.createDir:
        return Column(
          children: [
            Icon(
              Icons.create_new_folder,
              color: Theme.of(context).tabBarTheme.labelColor,
            ),
            Text(translate('Create Folder'))
          ],
        );
      case CmFileAction.rename:
        return Column(
          children: [
            Icon(
              Icons.drive_file_move_outlined,
              color: Theme.of(context).tabBarTheme.labelColor,
            ),
            Text(translate('Rename'))
          ],
        );
    }
  }

  Widget statusList() {
    return PreferredSize(
      preferredSize: const Size(200, double.infinity),
      child: Container(
          padding: const EdgeInsets.all(12.0),
          child: Obx(
            () {
              final jobTable = gFFI.cmFileModel.currentJobTable;
              statusListView(List<CmFileLog> jobs) => ListView.builder(
                    controller: ScrollController(),
                    itemBuilder: (BuildContext context, int index) {
                      final item = jobs[index];
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 5),
                        child: generateCard(
                          Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Row(
                                crossAxisAlignment: CrossAxisAlignment.center,
                                children: [
                                  SizedBox(
                                    width: 50,
                                    child: iconLabel(item),
                                  ).paddingOnly(left: 15),
                                  const SizedBox(
                                    width: 16.0,
                                  ),
                                  Expanded(
                                    child: Column(
                                      mainAxisSize: MainAxisSize.min,
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Text(
                                          item.fileName,
                                        ).paddingSymmetric(vertical: 10),
                                        if (item.totalSize > 0)
                                          Text(
                                            '${translate("Total")} ${readableFileSize(item.totalSize.toDouble())}',
                                            style: TextStyle(
                                              fontSize: 12,
                                              color: MyTheme.darkGray,
                                            ),
                                          ),
                                        if (item.totalSize > 0)
                                          Offstage(
                                            offstage: item.state !=
                                                JobState.inProgress,
                                            child: Text(
                                              '${translate("Speed")} ${readableFileSize(item.speed)}/s',
                                              style: TextStyle(
                                                fontSize: 12,
                                                color: MyTheme.darkGray,
                                              ),
                                            ),
                                          ),
                                        Offstage(
                                          offstage: !(item.isTransfer() &&
                                              item.state !=
                                                  JobState.inProgress),
                                          child: Text(
                                            translate(
                                              item.display(),
                                            ),
                                            style: TextStyle(
                                              fontSize: 12,
                                              color: MyTheme.darkGray,
                                            ),
                                          ),
                                        ),
                                        if (item.totalSize > 0)
                                          Offstage(
                                            offstage: item.state !=
                                                JobState.inProgress,
                                            child: LinearPercentIndicator(
                                              padding:
                                                  EdgeInsets.only(right: 15),
                                              animateFromLastPercent: true,
                                              center: Text(
                                                '${(item.finishedSize / item.totalSize * 100).toStringAsFixed(0)}%',
                                              ),
                                              barRadius: Radius.circular(15),
                                              percent: item.finishedSize /
                                                  item.totalSize,
                                              progressColor: MyTheme.accent,
                                              backgroundColor:
                                                  Theme.of(context).hoverColor,
                                              lineHeight:
                                                  kDesktopFileTransferRowHeight,
                                            ).paddingSymmetric(vertical: 15),
                                          ),
                                      ],
                                    ),
                                  ),
                                  Row(
                                    mainAxisAlignment: MainAxisAlignment.end,
                                    children: [],
                                  ),
                                ],
                              ),
                            ],
                          ).paddingSymmetric(vertical: 10),
                        ),
                      );
                    },
                    itemCount: jobTable.length,
                  );

              return jobTable.isEmpty
                  ? generateCard(
                      Center(
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            SvgPicture.asset(
                              "assets/transfer.svg",
                              colorFilter: svgColor(
                                  Theme.of(context).tabBarTheme.labelColor),
                              height: 40,
                            ).paddingOnly(bottom: 10),
                            Text(
                              translate("No transfers in progress"),
                              textAlign: TextAlign.center,
                              textScaler: TextScaler.linear(1.20),
                              style: TextStyle(
                                  color:
                                      Theme.of(context).tabBarTheme.labelColor),
                            ),
                          ],
                        ),
                      ),
                    )
                  : statusListView(jobTable);
            },
          )),
    );
  }
}
