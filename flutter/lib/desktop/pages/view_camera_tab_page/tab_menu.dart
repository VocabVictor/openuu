part of 'view_camera_tab_page.dart';

extension _ViewCameraTabMenu on _ViewCameraTabPageState {
  // Note: Some dup code to ../widgets/remote_toolbar
  Widget _tabMenuBuilder(String key, CancelFunc cancelFunc) {
    final List<MenuEntryBase<String>> menu = [];
    const EdgeInsets padding = EdgeInsets.only(left: 8.0, right: 5.0);
    final viewCameraPage = tabController.state.value.tabs
        .firstWhere((tab) => tab.key == key)
        .page as ViewCameraPage;
    final ffi = viewCameraPage.ffi;
    final sessionId = ffi.sessionId;
    final toolbarState = viewCameraPage.toolbarState;
    menu.addAll([
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Obx(() => Text(
              translate(
                  toolbarState.hide.isTrue ? 'Show Toolbar' : 'Hide Toolbar'),
              style: style,
            )),
        proc: () {
          toolbarState.switchHide(sessionId);
          cancelFunc();
        },
        padding: padding,
      ),
    ]);

    if (tabController.state.value.tabs.length > 1) {
      final splitAction = MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Move tab to new window'),
          style: style,
        ),
        proc: () async {
          await DesktopMultiWindow.invokeMethod(
              kMainWindowId,
              kWindowEventMoveTabToNewWindow,
              '${windowId()},$key,$sessionId,ViewCamera');
          cancelFunc();
        },
        padding: padding,
      );
      menu.insert(1, splitAction);
    }

    menu.addAll([
      MenuEntryDivider<String>(),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Copy Fingerprint'),
          style: style,
        ),
        proc: () => onCopyFingerprint(FingerprintState.find(key).value),
        padding: padding,
        dismissOnClicked: true,
        dismissCallback: cancelFunc,
      ),
      MenuEntryButton<String>(
        childBuilder: (TextStyle? style) => Text(
          translate('Close'),
          style: style,
        ),
        proc: () async {
          if (await desktopTryShowTabAuditDialogCloseCancelled(
            id: key,
            tabController: tabController,
          )) {
            return;
          }
          tabController.closeBy(key);
          cancelFunc();
        },
        padding: padding,
      )
    ]);

    return mod_menu.PopupMenu<String>(
      items: menu
          .map((entry) => entry.build(
              context,
              const MenuConfig(
                commonColor: _MenuTheme.blueColor,
                height: _MenuTheme.height,
                dividerHeight: _MenuTheme.dividerHeight,
              )))
          .expand((i) => i)
          .toList(),
    );
  }

  void onRemoveId(String id) async {
    if (tabController.state.value.tabs.isEmpty) {
      // Keep calling until the window status is hidden.
      //
      // Workaround for Windows:
      // If you click other buttons and close in msgbox within a very short period of time, the close may fail.
      // `await WindowController.fromWindowId(windowId()).close();`.
      Future<void> loopCloseWindow() async {
        int c = 0;
        final windowController = WindowController.fromWindowId(windowId());
        while (c < 20 &&
            tabController.state.value.tabs.isEmpty &&
            (!await windowController.isHidden())) {
          await windowController.close();
          await Future.delayed(Duration(milliseconds: 100));
          c++;
        }
      }

      loopCloseWindow();
    }
    ConnectionTypeState.delete(id);
    _update_remote_count();
  }

  int windowId() {
    return widget.params["windowId"];
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.length;
    if (connLength == 1) {
      if (await desktopTryShowTabAuditDialogCloseCancelled(
        id: tabController.state.value.tabs[0].key,
        tabController: tabController,
      )) {
        return false;
      }
    }
    if (connLength <= 1) {
      tabController.clear();
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
        tabController.clear();
      }
      return res;
    }
  }
}
