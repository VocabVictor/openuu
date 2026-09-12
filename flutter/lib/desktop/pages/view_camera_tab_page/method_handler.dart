part of 'view_camera_tab_page.dart';

extension _ViewCameraTabMethodHandler on _ViewCameraTabPageState {
  Future<dynamic> _remoteMethodHandler(call, fromWindowId) async {
    debugPrint(
        "[View Camera Page] call ${call.method} with args ${call.arguments} from window $fromWindowId");

    dynamic returnValue;
    // for simplify, just replace connectionId
    if (call.method == kWindowEventNewViewCamera) {
      final args = jsonDecode(call.arguments);
      final id = args['id'];
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
            WindowType.ViewCamera, display, screenRect);
        Future.delayed(Duration(milliseconds: isWindows ? 100 : 0), () async {
          await windowOnTop(windowId());
        });
      });
      ConnectionTypeState.init(id);
      tabController.add(TabInfo(
        key: id,
        label: id,
        selectedIcon: _ViewCameraTabPageState.selectedIcon,
        unselectedIcon: _ViewCameraTabPageState.unselectedIcon,
        onTabCloseButton: () async {
          if (await desktopTryShowTabAuditDialogCloseCancelled(
            id: id,
            tabController: tabController,
          )) {
            return;
          }
          tabController.closeBy(id);
        },
        page: ViewCameraPage(
          key: ValueKey(id),
          id: id,
          sessionId: sessionId == null ? null : SessionID(sessionId),
          tabWindowId: tabWindowId,
          display: display,
          displays: displays?.cast<int>(),
          password: args['password'],
          toolbarState: ToolbarState(),
          tabController: tabController,
          connToken: args['connToken'],
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
      final jumpOk =
          tabController.jumpToByKeyAndDisplay(id, display, isCamera: true);
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
          .map((e) => '${e.key},${(e.page as ViewCameraPage).ffi.sessionId}')
          .toList()
          .join(';');
    } else if (call.method == kWindowEventGetCachedSessionData) {
      // Ready to show new window and close old tab.
      final args = jsonDecode(call.arguments);
      final id = args['id'];
      final close = args['close'];
      try {
        final viewCameraPage = tabController.state.value.tabs
            .firstWhere((tab) => tab.key == id)
            .page as ViewCameraPage;
        returnValue = viewCameraPage.ffi.ffiModel.cachedPeerData.toString();
      } catch (e) {
        debugPrint('Failed to get cached session data: $e');
      }
      if (close && returnValue != null) {
        closeSessionOnDispose[id] = false;
        tabController.closeBy(id);
      }
    } else if (call.method == kWindowEventRemoteWindowCoords) {
      final viewCameraPage =
          tabController.state.value.selectedTabInfo.page as ViewCameraPage;
      final ffi = viewCameraPage.ffi;
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
