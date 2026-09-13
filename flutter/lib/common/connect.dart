import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../consts.dart';
import '../common/widgets/login.dart';
import '../mobile/pages/file_manager_page.dart';
import '../mobile/pages/remote_page.dart';
import '../mobile/pages/view_camera_page.dart';
import '../mobile/pages/terminal_page.dart';
import '../models/platform_model.dart';

import 'ffi_options.dart';
import 'globals.dart';
import 'windows_misc.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/user_model.dart';

closeConnection({String? id}) {
  if (isAndroid || isIOS) {
    () async {
      await SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
          overlays: SystemUiOverlay.values);
      gFFI.chatModel.hideChatOverlay();
      Navigator.popUntil(globalKey.currentContext!, ModalRoute.withName("/"));
      stateGlobal.isInMainPage = true;
    }();
  } else {
    final controller = Get.find<DesktopTabController>();
    if (controller.tabType == DesktopTabType.terminal &&
        controller.onCloseWindow != null) {
      // Terminal windows are scoped to one peer. The optional id passed to
      // closeConnection() is that peer id, not a terminal tab key
      // (${peerId}_${terminalId}). Closing from terminal dialogs should close
      // the peer's whole terminal window, including all terminal tabs.
      unawaited(controller.onCloseWindow!().catchError((e, _) {
        debugPrint('[closeConnection] Failed to close terminal window: $e');
      }));
      return;
    }
    controller.closeBy(id);
  }
}

connectMainDesktop(String id,
    {String? quickLaunch,
    bool viewOnly = false,
    required bool isFileTransfer,
    required bool isViewCamera,
    required bool isTerminal,
    required bool isTcpTunneling,
    required bool isRDP,
    bool? forceRelay,
    String? password,
    String? connToken,
    bool? isSharedPassword}) async {
  if (isFileTransfer) {
    await rustDeskWinManager.newFileTransfer(id,
        password: password,
        isSharedPassword: isSharedPassword,
        connToken: connToken,
        forceRelay: forceRelay);
  } else if (isViewCamera) {
    await rustDeskWinManager.newViewCamera(id,
        password: password,
        isSharedPassword: isSharedPassword,
        connToken: connToken,
        forceRelay: forceRelay);
  } else if (isTcpTunneling || isRDP) {
    await rustDeskWinManager.newPortForward(id, isRDP,
        password: password,
        isSharedPassword: isSharedPassword,
        connToken: connToken,
        forceRelay: forceRelay);
  } else if (isTerminal) {
    await rustDeskWinManager.newTerminal(id,
        password: password,
        isSharedPassword: isSharedPassword,
        connToken: connToken,
        forceRelay: forceRelay);
  } else {
    await rustDeskWinManager.newRemoteDesktop(id,
        quickLaunch: quickLaunch,
        viewOnly: viewOnly,
        password: password,
        isSharedPassword: isSharedPassword,
        forceRelay: forceRelay);
  }
}

/// Connect to a peer with [id].
/// If [isFileTransfer], starts a session only for file transfer.
/// If [isViewCamera], starts a session only for view camera.
/// If [isTcpTunneling], starts a session only for tcp tunneling.
/// If [isRDP], starts a session only for rdp.
connect(BuildContext context, String id,
    {bool isFileTransfer = false,
    bool viewOnly = false,
    String? quickLaunch,
    bool isViewCamera = false,
    bool isTerminal = false,
    bool isTcpTunneling = false,
    bool isRDP = false,
    bool forceRelay = false,
    String? password,
    String? connToken,
    bool? isSharedPassword}) async {
  if (id == '') return;
  if (!await gFFI.userModel.validateSession()) { await loginDialog(); return; }
  if (!isDesktop || desktopType == DesktopType.main) {
    try {
      if (Get.isRegistered<IDTextEditingController>()) {
        final idController = Get.find<IDTextEditingController>();
        idController.text = formatID(id);
      }
      if (Get.isRegistered<TextEditingController>()) {
        final fieldTextEditingController = Get.find<TextEditingController>();
        fieldTextEditingController.text = formatID(id);
      }
    } catch (_) {}
  }
  id = id.replaceAll(' ', '');
  final oldId = id;
  id = await bind.mainHandleRelayId(id: id);
  forceRelay = id != oldId || forceRelay;
  assert(!(isFileTransfer && isTcpTunneling && isRDP),
      "more than one connect type");

  if (isDesktop) {
    if (desktopType == DesktopType.main) {
      await connectMainDesktop(
        id,
        quickLaunch: quickLaunch,
        viewOnly: viewOnly,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isTerminal: isTerminal,
        isTcpTunneling: isTcpTunneling,
        isRDP: isRDP,
        password: password,
        isSharedPassword: isSharedPassword,
        forceRelay: forceRelay,
      );
    } else {
      await rustDeskWinManager.call(WindowType.Main, kWindowConnect, {
        'id': id,
        'quickLaunch': quickLaunch,
        'viewOnly': viewOnly,
        'isFileTransfer': isFileTransfer,
        'isViewCamera': isViewCamera,
        'isTerminal': isTerminal,
        'isTcpTunneling': isTcpTunneling,
        'isRDP': isRDP,
        'password': password,
        'isSharedPassword': isSharedPassword,
        'forceRelay': forceRelay,
        'connToken': connToken,
      });
    }
  } else {
    if (isFileTransfer) {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => FileManagerPage(
              id: id,
              password: password,
              isSharedPassword: isSharedPassword,
              forceRelay: forceRelay),
        ),
      );
    } else if (isViewCamera) {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => ViewCameraPage(
              id: id,
              password: password,
              isSharedPassword: isSharedPassword,
              forceRelay: forceRelay),
        ),
      );
    } else if (isTerminal) {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => TerminalPage(
            id: id,
            password: password,
            isSharedPassword: isSharedPassword,
            forceRelay: forceRelay,
          ),
        ),
      );
    } else {
      Navigator.push(
        context,
        MaterialPageRoute(
          builder: (BuildContext context) => RemotePage(
              id: id,
              password: password,
              isSharedPassword: isSharedPassword,
              forceRelay: forceRelay),
        ),
      );
    }
    stateGlobal.isInMainPage = false;
  }

  FocusScopeNode currentFocus = FocusScope.of(context);
  if (!currentFocus.hasPrimaryFocus) {
    currentFocus.unfocus();
  }
}
