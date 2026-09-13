part of 'tabbar_widget.dart';

class WindowActionPanel extends StatefulWidget {
  final bool isMainWindow;
  final Rx<DesktopTabState> state;
  final DesktopTabController tabController;

  final bool showMinimize;
  final bool showMaximize;
  final bool showClose;
  final Widget? tail;
  final Future<bool> Function()? onClose;

  final RxList<String> invisibleTabKeys;
  final LabelGetter? labelGetter;

  const WindowActionPanel(
      {Key? key,
      required this.isMainWindow,
      required this.state,
      required this.tabController,
      required this.invisibleTabKeys,
      this.tail,
      this.showMinimize = true,
      this.showMaximize = true,
      this.showClose = true,
      this.onClose,
      this.labelGetter})
      : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return WindowActionPanelState();
  }
}

class WindowActionPanelState extends State<WindowActionPanel> {
  bool showTabDowndown() {
    return widget.tabController.state.value.tabs.length > 1 &&
        (widget.tabController.tabType == DesktopTabType.remoteScreen ||
            widget.tabController.tabType == DesktopTabType.fileTransfer ||
            widget.tabController.tabType == DesktopTabType.viewCamera ||
            widget.tabController.tabType == DesktopTabType.portForward ||
            widget.tabController.tabType == DesktopTabType.cm);
  }

  List<String> existingInvisibleTab() {
    return widget.invisibleTabKeys
        .where((key) =>
            widget.tabController.state.value.tabs.any((tab) => tab.key == key))
        .toList();
  }

  @override
  Widget build(BuildContext context) {
    final session = _isSessionTab(widget.tabController.tabType);
    return Row(
      mainAxisAlignment: MainAxisAlignment.end,
      children: [
        Obx(() {
          if (showTabDowndown() && existingInvisibleTab().isNotEmpty) {
            return _TabDropDownButton(
                controller: widget.tabController,
                labelGetter: widget.labelGetter,
                tabkeys: existingInvisibleTab());
          } else {
            return Offstage();
          }
        }),
        if (widget.tail != null) widget.tail!,
        if (!kUseCompatibleUiMode)
          Row(
            children: [
              if (widget.showMinimize && !isMacOS)
                ActionIcon(
                  message: 'Minimize',
                  icon: IconFont.min,
                  onTap: () {
                    if (widget.isMainWindow) {
                      windowManager.minimize();
                    } else {
                      WindowController.fromWindowId(kWindowId!).minimize();
                    }
                  },
                  isClose: false,
                  session: session,
                ),
              if (widget.showMaximize && !isMacOS)
                Obx(() => ActionIcon(
                      message: stateGlobal.isMaximized.isTrue
                          ? 'Restore'
                          : 'Maximize',
                      icon: stateGlobal.isMaximized.isTrue
                          ? IconFont.restore
                          : IconFont.max,
                      onTap: bind.isIncomingOnly() && isInHomePage()
                          ? null
                          : _toggleMaximize,
                      isClose: false,
                      session: session,
                    )),
              if (widget.showClose && !isMacOS)
                ActionIcon(
                  message: 'Close',
                  icon: IconFont.close,
                  onTap: () async {
                    final res = await widget.onClose?.call() ?? true;
                    if (res) {
                      // hide for all window
                      // note: the main window can be restored by tray icon
                      Future.delayed(Duration.zero, () async {
                        if (widget.isMainWindow) {
                          await windowManager.close();
                        } else {
                          await WindowController.fromWindowId(kWindowId!)
                              .close();
                        }
                      });
                    }
                  },
                  isClose: true,
                  session: session,
                )
            ],
          ),
      ],
    );
  }

  void _toggleMaximize() {
    toggleMaximize(widget.isMainWindow).then((maximize) {
      // update state for sub window, wc.unmaximize/maximize() will not invoke onWindowMaximize/Unmaximize
      stateGlobal.setMaximized(maximize);
    });
  }
}

void startDragging(bool isMainWindow) {
  if (isMainWindow) {
    windowManager.startDragging();
  } else {
    WindowController.fromWindowId(kWindowId!).startDragging();
  }
}

void setMovable(bool isMainWindow, bool movable) {
  if (isMainWindow) {
    windowManager.setMovable(movable);
  } else {
    WindowController.fromWindowId(kWindowId!).setMovable(movable);
  }
}

/// return true -> window will be maximize
/// return false -> window will be unmaximize
Future<bool> toggleMaximize(bool isMainWindow) async {
  if (isMainWindow) {
    if (await windowManager.isMaximized()) {
      windowManager.unmaximize();
      return false;
    } else {
      windowManager.maximize();
      return true;
    }
  } else {
    final wc = WindowController.fromWindowId(kWindowId!);
    if (await wc.isMaximized()) {
      wc.unmaximize();
      return false;
    } else {
      wc.maximize();
      return true;
    }
  }
}

Future<bool> closeConfirmDialog() async {
  var confirm = true;
  final res = await gFFI.dialogManager.show<bool>((setState, close, context) {
    submit() {
      String value = bool2option(kOptionEnableConfirmClosingTabs, confirm);
      bind.mainSetLocalOption(
          key: kOptionEnableConfirmClosingTabs, value: value);
      close(true);
    }

    return CustomAlertDialog(
      title: Row(children: [
        const Icon(Icons.warning_amber_sharp,
            color: Colors.redAccent, size: 28),
        const SizedBox(width: 10),
        Text(translate("Warning")),
      ]),
      content: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(translate("Disconnect all devices?")),
            CheckboxListTile(
              contentPadding: const EdgeInsets.all(0),
              dense: true,
              controlAffinity: ListTileControlAffinity.leading,
              title: Text(
                translate("Confirm before closing multiple tabs"),
              ),
              value: confirm,
              onChanged: (v) {
                if (v == null) return;
                setState(() => confirm = v);
              },
            )
          ]),
      // confirm checkbox
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
  return res == true;
}
