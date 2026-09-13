part of 'tabbar_widget.dart';

// ignore: must_be_immutable
class _DesktopTabState extends State<DesktopTab>
    with MultiWindowListener, WindowListener {
  Timer? _macOSCheckRestoreTimer;
  int _macOSCheckRestoreCounter = 0;

  bool get showLogo => widget.showLogo;
  bool get showTitle => widget.showTitle;
  bool get showMinimize => widget.showMinimize;
  bool get showMaximize => widget.showMaximize;
  bool get showClose => widget.showClose;
  Widget Function(Widget pageView)? get pageViewBuilder =>
      widget.pageViewBuilder;
  TabMenuBuilder? get tabMenuBuilder => widget.tabMenuBuilder;
  Widget? get tail => widget.tail;
  Future<bool> Function()? get onWindowCloseButton =>
      widget.onWindowCloseButton;
  TabBuilder? get tabBuilder => widget.tabBuilder;
  LabelGetter? get labelGetter => widget.labelGetter;
  double? get maxLabelWidth => widget.maxLabelWidth;
  Color? get selectedTabBackgroundColor => widget.selectedTabBackgroundColor;
  Color? get unSelectedTabBackgroundColor =>
      widget.unSelectedTabBackgroundColor;
  Color? get selectedBorderColor => widget.selectedBorderColor;
  DesktopTabController get controller => widget.controller;
  RxList<String> get invisibleTabKeys => widget.invisibleTabKeys;
  Debouncer get _scrollDebounce => widget._scrollDebounce;

  Rx<DesktopTabState> get state => controller.state;

  DesktopTabType get tabType => controller.tabType;
  bool get isMainWindow =>
      tabType == DesktopTabType.main ||
      tabType == DesktopTabType.cm ||
      tabType == DesktopTabType.install;

  _DesktopTabState() : super();

  static RxString tablabelGetter(String peerId) {
    final alias = bind.mainGetPeerOptionSync(id: peerId, key: 'alias');
    return RxString(getDesktopTabLabel(peerId, alias));
  }

  @override
  void initState() {
    super.initState();
    DesktopMultiWindow.addListener(this);
    windowManager.addListener(this);

    Future.delayed(Duration(milliseconds: 500), () {
      if (isMainWindow) {
        windowManager.isMaximized().then((maximized) {
          if (stateGlobal.isMaximized.value != maximized) {
            WidgetsBinding.instance.addPostFrameCallback(
                (_) => setState(() => stateGlobal.setMaximized(maximized)));
          }
        });
      } else {
        final wc = WindowController.fromWindowId(kWindowId!);
        wc.isMaximized().then((maximized) {
          debugPrint("isMaximized $maximized");
          if (stateGlobal.isMaximized.value != maximized) {
            WidgetsBinding.instance.addPostFrameCallback(
                (_) => setState(() => stateGlobal.setMaximized(maximized)));
          }
        });
      }
    });
  }

  @override
  void dispose() {
    DesktopMultiWindow.removeListener(this);
    windowManager.removeListener(this);
    _macOSCheckRestoreTimer?.cancel();
    super.dispose();
  }

  void _setMaximized(bool maximize) {
    stateGlobal.setMaximized(maximize);
    _saveFrame();
    setState(() {});
  }

  @override
  void onWindowFocus() {
    stateGlobal.isFocused.value = true;
  }

  @override
  void onWindowBlur() {
    stateGlobal.isFocused.value = false;
  }

  @override
  void onWindowMinimize() {
    stateGlobal.setMinimized(true);
    stateGlobal.setMaximized(false);
    super.onWindowMinimize();
  }

  @override
  void onWindowMaximize() {
    stateGlobal.setMinimized(false);
    _setMaximized(true);
    super.onWindowMaximize();
  }

  @override
  void onWindowUnmaximize() {
    stateGlobal.setMinimized(false);
    _setMaximized(false);
    super.onWindowUnmaximize();
  }

  _saveFrame({bool? flush}) async {
    try {
      if (tabType == DesktopTabType.main) {
        await saveWindowPosition(WindowType.Main, flush: flush);
      } else if (kWindowType != null && kWindowId != null) {
        await saveWindowPosition(kWindowType!,
            windowId: kWindowId, flush: flush);
      }
    } catch (e) {
      debugPrint('Error saving window position: $e');
    }
  }

  @override
  void onWindowMoved() {
    if (kWindowType == WindowType.RemoteDesktop) noteSessionWindowFrameEvent();
    _saveFrame();
    super.onWindowMoved();
  }

  @override
  void onWindowResized() {
    if (kWindowType == WindowType.RemoteDesktop) noteSessionWindowFrameEvent();
    _saveFrame();
    super.onWindowResized();
  }

  @override
  void onWindowClose() async {
    mainWindowClose() async => await windowManager.hide();
    notMainWindowClose(WindowController windowController) async {
      if (controller.length != 0) {
        debugPrint("close not empty multiwindow from taskbar");
        if (isWindows) {
          await windowController.show();
          await windowController.focus();
          final res = await onWindowCloseButton?.call() ?? true;
          if (!res) return;
        }
        controller.clear();
      }
      await windowController.hide();
      await rustDeskWinManager
          .call(WindowType.Main, kWindowEventHide, {"id": kWindowId!});
    }

    macOSWindowClose(
      Future<bool> Function() checkFullscreen,
      Future<void> Function() closeFunc,
    ) async {
      _macOSCheckRestoreCounter = 0;
      _macOSCheckRestoreTimer =
          Timer.periodic(Duration(milliseconds: 30), (timer) async {
        _macOSCheckRestoreCounter++;
        if (!await checkFullscreen() || _macOSCheckRestoreCounter >= 30) {
          _macOSCheckRestoreTimer?.cancel();
          _macOSCheckRestoreTimer = null;
          Timer(Duration(milliseconds: 700), () async => await closeFunc());
        }
      });
    }

    await _saveFrame(flush: true);

    // hide window on close
    if (isMainWindow) {
      if (rustDeskWinManager.getActiveWindows().contains(kMainWindowId)) {
        await rustDeskWinManager.unregisterActiveWindow(kMainWindowId);
      }
      // macOS specific workaround, the window is not hiding when in fullscreen.
      if (isMacOS && await windowManager.isFullScreen()) {
        await windowManager.setFullScreen(false);
        await macOSWindowClose(
          () async => await windowManager.isFullScreen(),
          mainWindowClose,
        );
      } else {
        await mainWindowClose();
      }
    } else {
      // it's safe to hide the subwindow
      final controller = WindowController.fromWindowId(kWindowId!);
      if (isMacOS) {
        // onWindowClose() maybe called multiple times because of loopCloseWindow() in remote_tab_page.dart.
        // use ??=  to make sure the value is set on first call.

        if (await onWindowCloseButton?.call() ?? true) {
          if (await controller.isFullScreen()) {
            await controller.setFullscreen(false);
            stateGlobal.setFullscreen(false, procWnd: false);
            await macOSWindowClose(
              () async => await controller.isFullScreen(),
              () async => await notMainWindowClose(controller),
            );
          } else {
            await notMainWindowClose(controller);
          }
        }
      } else {
        await notMainWindowClose(controller);
      }
    }
    super.onWindowClose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(children: [
      Obx(() {
        if (stateGlobal.showTabBar.isTrue &&
            !(kUseCompatibleUiMode && isHideSingleItem())) {
          final showBottomDivider = _showTabBarBottomDivider(tabType);
          return SizedBox(
            height: _kTabBarHeight,
            child: Column(
              children: [
                SizedBox(
                  height:
                      showBottomDivider ? _kTabBarHeight - 1 : _kTabBarHeight,
                  child: _buildBar(),
                ),
                if (showBottomDivider)
                  const Divider(
                    height: 1,
                  ),
              ],
            ),
          );
        } else {
          return Offstage();
        }
      }),
      Expanded(
          child: pageViewBuilder != null
              ? pageViewBuilder!(_buildPageView())
              : _buildPageView())
    ]);
  }

  List<Widget> _tabWidgets = [];

}
