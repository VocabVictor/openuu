part of 'remote_tab_page.dart';

extension _ConnectionTabMethodHandler on _ConnectionTabPageState {
  Future<dynamic> _remoteMethodHandler(call, fromWindowId) async {
    debugPrint(
        "[Remote Page] call ${call.method} with args ${call.arguments} from window $fromWindowId");

    dynamic returnValue;
    // for simplify, just replace connectionId
    if (call.method == 'quick_launch') {
      final peerId = call.arguments['id'];
      for (final tab in tabController.state.value.tabs) {
        if (tab.key != peerId || tab.page is! RemotePage) continue;
        final page = tab.page as RemotePage;
        await windowOnTop(windowId());
        tabController.jumpToByKey(peerId);
        if (page.ffi.viewOnlySession || page.ffi.ffiModel.viewOnly || !page.ffi.ffiModel.pi.isSet.isTrue) {
          if (mounted) ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
            content: Text('请等待控制连接就绪；观看连接需先关闭。 / Wait for a control session; close any view-only session first.')));
        } else if (mounted) {
          showQuickLaunch(context, page.ffi, call.arguments['app'] ?? '');
        }
        return true;
      }
      return false;
    }
    if (call.method == kWindowEventNewRemoteDesktop) {
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final switchUuid = args['switch_uuid'];
      final sessionId = args['session_id'];
      final tabWindowId = args['tab_window_id'];
      final display = args['display'];
      final displays = args['displays'];
      final screenRect = parseParamScreenRect(args);
      final prePeerCount = tabController.length;
      Future.delayed(Duration.zero, () async {
        if (stateGlobal.fullscreen.isTrue) {
          await WindowController.fromWindowId(windowId()).setFullscreen(false);
          stateGlobal.setFullscreen(false, procWnd: false);
        }
        await setNewConnectWindowFrame(windowId(), id!, prePeerCount,
            WindowType.RemoteDesktop, display, screenRect);
        Future.delayed(Duration(milliseconds: isWindows ? 100 : 0), () async {
          await windowOnTop(windowId());
        });
      });
      ConnectionTypeState.init(id);
      tabController.add(TabInfo(
        key: id,
        label: id,
        selectedIcon: _ConnectionTabPageState.selectedIcon,
        unselectedIcon: _ConnectionTabPageState.unselectedIcon,
        onTabCloseButton: () async {
          if (await desktopTryShowTabAuditDialogCloseCancelled(
            id: id,
            tabController: tabController,
          )) {
            return;
          }
          tabController.closeBy(id);
        },
        page: RemotePage(
          key: ValueKey(id),
          id: id,
          sessionId: sessionId == null ? null : SessionID(sessionId),
          tabWindowId: tabWindowId,
          display: display,
          displays: displays?.cast<int>(),
          password: args['password'],
          toolbarState: ToolbarState(),
          tabController: tabController,
          switchUuid: switchUuid,
          quickLaunch: args['quickLaunch'],
          viewOnly: args['viewOnly'] == true,
          forceRelay: args['forceRelay'],
          isSharedPassword: args['isSharedPassword'],
        ),
      ));
    } else if (call.method == kWindowDisableGrabKeyboard) {
      // ???
    } else if (call.method == "onDestroy") {
      tabController.clear();
    } else if (call.method == kWindowActionRebuild) {
      reloadCurrentWindow();
    } else if (call.method == kWindowEventActiveSession) {
      final jumpOk = tabController.jumpToByKey(call.arguments);
      if (jumpOk) {
        windowOnTop(windowId());
      }
      return jumpOk;
    } else if (call.method == kWindowEventActiveDisplaySession) {
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final display = args['display'];
      final jumpOk = tabController.jumpToByKeyAndDisplay(id, display);
      if (jumpOk) {
        windowOnTop(windowId());
      }
      return jumpOk;
    } else if (call.method == kWindowEventGetRemoteList) {
      return tabController.state.value.tabs
          .map((e) => e.key)
          .toList()
          .join(',');
    } else if (call.method == kWindowEventGetSessionIdList) {
      return tabController.state.value.tabs
          .map((e) => '${e.key},${(e.page as RemotePage).ffi.sessionId}')
          .toList()
          .join(';');
    } else if (call.method == kWindowEventGetCachedSessionData) {
      // Ready to show new window and close old tab.
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final close = args['close'];
      RemotePage? remotePage;
      try {
        remotePage = tabController.state.value.tabs
            .firstWhere((tab) => tab.key == id)
            .page as RemotePage;
        returnValue = remotePage.ffi.ffiModel.cachedPeerData.toString();
      } catch (e) {
        debugPrint('Failed to get cached session data: $e');
      }
      if (close && returnValue != null && remotePage != null) {
        remotePage.releaseMacOSInputForTabTransfer();
        closeSessionOnDispose[id] = false;
        tabController.closeBy(id);
      }
    } else if (call.method == kWindowEventRemoteWindowCoords) {
      final remotePage =
          tabController.state.value.selectedTabInfo.page as RemotePage;
      final ffi = remotePage.ffi;
      final displayRect = ffi.ffiModel.displaysRect();
      if (displayRect != null) {
        final wc = WindowController.fromWindowId(windowId());
        Rect? frame;
        try {
          frame = await wc.getFrame();
        } catch (e) {
          debugPrint(
              "Failed to get frame of window $windowId, it may be hidden");
        }
        if (frame != null) {
          ffi.cursorModel.moveLocal(0, 0);
          final coords = RemoteWindowCoords(
              frame,
              CanvasCoords.fromCanvasModel(ffi.canvasModel),
              CursorCoords.fromCursorModel(ffi.cursorModel),
              displayRect);
          returnValue = jsonEncode(coords.toJson());
        }
      }
    } else if (call.method == kWindowEventSetFullscreen) {
      stateGlobal.setFullscreen(call.arguments == 'true');
    }
    _update_remote_count();
    return returnValue;
  }
}
