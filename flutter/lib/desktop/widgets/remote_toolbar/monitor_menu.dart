part of 'remote_toolbar.dart';

class _MonitorCycle {
  final String id;
  final FFI ffi;
  const _MonitorCycle(this.id, this.ffi);

  PeerInfo get _pi => ffi.ffiModel.pi;
  int get total => _pi.displays.length;
  int get _current => CurrentDisplayState.find(id).value;
  bool get _inRange => _current >= 0 && _current < total;

  String get label => _inRange ? '${_current + 1}' : '*';
  String get tooltip => '${translate('Switch display')} ($label/$total)';

  void next() {
    final t = total;
    if (t < 2) return;
    final from = _inRange ? _current : -1;
    final target = (from + 1) % t;
    final isChooseDisplayToOpenInNewWindow = _pi.isSupportMultiDisplay &&
        bind.sessionGetDisplaysAsIndividualWindows(sessionId: ffi.sessionId) ==
            'Y';
    if (isChooseDisplayToOpenInNewWindow) {
      openMonitorInNewTabOrWindow(target, ffi.id, _pi);
    } else {
      openMonitorInTheSameTab(target, ffi, _pi, updateCursorPos: false);
    }
  }
}

class _MonitorMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  final _ToolbarEdge edge;
  final Function(VoidCallback) setRemoteState;
  const _MonitorMenu({
    Key? key,
    required this.id,
    required this.ffi,
    required this.edge,
    required this.setRemoteState,
  }) : super(key: key);

  bool get showMonitorsToolbar =>
      bind.mainGetUserDefaultOption(key: kKeyShowMonitorsToolbar) == 'Y';

  bool get supportIndividualWindows =>
      ffi.ffiModel.pi.isSupportMultiDisplay;

  @override
  Widget build(BuildContext context) {
    final child = showMonitorsToolbar
        ? buildMultiMonitorMenu(context)
        : Obx(() => buildMonitorMenu(context));
    final quarterTurns = _monitorMenuQuarterTurns(edge);
    if (quarterTurns == 0) return child;
    return RotatedBox(
      quarterTurns: quarterTurns,
      child: child,
    );
  }

  Widget buildMonitorMenu(BuildContext context) {
    final width = SimpleWrapper<double>(0);
    final monitorsIcon =
        globalMonitorsWidget(context, width, UiColor.of(context).surface,
            UiColor.of(context).muted);
    return _IconSubmenuButton(
        tooltip: 'Select Monitor',
        icon: monitorsIcon,
        ffi: ffi,
        width: width.value,
        color: _ToolbarTheme.blueColor,
        hoverColor: _ToolbarTheme.hoverBlueColor(context),
        menuStyle: MenuStyle(
            padding:
                MaterialStatePropertyAll(EdgeInsets.symmetric(horizontal: 6))),
        menuChildrenGetter: (_) => [buildMonitorSubmenuWidget(context)]);
  }

  Widget buildMultiMonitorMenu(BuildContext context) {
    return Row(children: buildMonitorList(context, true));
  }

  Widget buildMonitorSubmenuWidget(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Row(children: buildMonitorList(context, false)),
        supportIndividualWindows ? Divider() : Offstage(),
        supportIndividualWindows ? chooseDisplayBehavior() : Offstage(),
      ],
    );
  }

  Widget chooseDisplayBehavior() {
    final value =
        bind.sessionGetDisplaysAsIndividualWindows(sessionId: ffi.sessionId) ==
            'Y';
    return CkbMenuButton(
        value: value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionSetDisplaysAsIndividualWindows(
              sessionId: ffi.sessionId, value: value ? 'Y' : 'N');
        },
        ffi: ffi,
        child: Text(translate('Show displays as individual windows')));
  }

  buildOneMonitorButton(BuildContext context, i, curDisplay) => Text(
        '${i + 1}',
        style: TextStyle(
          color: i == curDisplay
              ? UiColor.of(context).primary
              : UiColor.of(context).muted,
          fontSize: 12,
          fontWeight: FontWeight.bold,
        ),
      );

  List<Widget> buildMonitorList(BuildContext context, bool isMulti) {
    final List<Widget> monitorList = [];
    final pi = ffi.ffiModel.pi;

    buildMonitorButton(int i) => Obx(() {
          RxInt display = CurrentDisplayState.find(id);

          final isAllMonitors = i == kAllDisplayValue;
          final width = SimpleWrapper<double>(0);
          Widget? monitorsIcon;
          if (isAllMonitors) {
            monitorsIcon = globalMonitorsWidget(context, width,
                UiColor.of(context).textSecondary, UiColor.of(context).primary);
          }
          return _IconMenuButton(
            tooltip: isMulti
                ? ''
                : isAllMonitors
                    ? 'All monitors'
                    : '#{${i + 1}} monitor',
            hMargin: isMulti ? null : 6,
            vMargin: isMulti ? null : 12,
            topLevel: false,
            color: i == display.value
                ? _ToolbarTheme.activeColor(context)
                : _ToolbarTheme.blueColor,
            hoverColor: i == display.value
                ? _ToolbarTheme.hoverActiveColor(context)
                : _ToolbarTheme.hoverBlueColor(context),
            width: isAllMonitors ? width.value : null,
            icon: isAllMonitors
                ? monitorsIcon
                : Container(
                    alignment: AlignmentDirectional.center,
                    constraints:
                        const BoxConstraints(minHeight: _ToolbarTheme.height),
                    child: Stack(
                      alignment: Alignment.center,
                      children: [
                        SvgPicture.asset(
                          "assets/screen.svg",
                          colorFilter: ColorFilter.mode(
                              UiColor.of(context).surface, BlendMode.srcIn),
                        ),
                        Obx(() =>
                            buildOneMonitorButton(context, i, display.value)),
                      ],
                    ),
                  ),
            onPressed: () => onPressed(i, pi, isMulti),
          );
        });

    for (int i = 0; i < pi.displays.length; i++) {
      monitorList.add(buildMonitorButton(i));
    }
    if (supportIndividualWindows && pi.displays.length > 1) {
      monitorList.add(buildMonitorButton(kAllDisplayValue));
    }
    return monitorList;
  }

  globalMonitorsWidget(BuildContext context, SimpleWrapper<double> width,
      Color activeTextColor, Color activeBgColor) {
    final pal = UiColor.of(context);
    getMonitors() {
      final pi = ffi.ffiModel.pi;
      RxInt display = CurrentDisplayState.find(id);
      final rect = ffi.ffiModel.globalDisplaysRect();
      if (rect == null) {
        return Offstage();
      }

      final scale = _ToolbarTheme.buttonSize / rect.height * 0.75;
      final height = rect.height * scale;
      final startY = (_ToolbarTheme.buttonSize - height) * 0.5;
      final startX = startY;

      final children = <Widget>[];
      for (var i = 0; i < pi.displays.length; i++) {
        final d = pi.displays[i];
        double s = d.scale;
        int dWidth = d.width.toDouble() ~/ s;
        int dHeight = d.height.toDouble() ~/ s;
        final fontSize = (dWidth * scale < dHeight * scale
                ? dWidth * scale
                : dHeight * scale) *
            0.65;
        children.add(Positioned(
          left: (d.x - rect.left) * scale + startX,
          top: (d.y - rect.top) * scale + startY,
          width: dWidth * scale,
          height: dHeight * scale,
          child: Container(
            decoration: BoxDecoration(
              border: Border.all(
                color: pal.border,
                width: 1.0,
              ),
              color: display.value == i ? activeBgColor : pal.surface,
            ),
            child: Center(
                child: Text(
              '${i + 1}',
              style: TextStyle(
                color: display.value == i ? activeTextColor : pal.muted,
                fontSize: fontSize,
                fontWeight: FontWeight.bold,
              ),
            )),
          ),
        ));
      }
      width.value = rect.width * scale + startX * 2;
      return SizedBox(
        width: width.value,
        height: height + startY * 2,
        child: Stack(
          children: children,
        ),
      );
    }

    return Stack(
      alignment: Alignment.center,
      children: [
        SizedBox(height: _ToolbarTheme.buttonSize),
        getMonitors(),
      ],
    );
  }

  onPressed(int i, PeerInfo pi, bool isMulti) {
    if (!isMulti) {
      // If show monitors in toolbar(`buildMultiMonitorMenu()`), then the menu will dismiss automatically.
      _menuDismissCallback(ffi);
    }
    RxInt display = CurrentDisplayState.find(id);
    if (display.value != i) {
      final isChooseDisplayToOpenInNewWindow = pi.isSupportMultiDisplay &&
          bind.sessionGetDisplaysAsIndividualWindows(
                  sessionId: ffi.sessionId) ==
              'Y';
      if (isChooseDisplayToOpenInNewWindow) {
        openMonitorInNewTabOrWindow(i, ffi.id, pi);
      } else {
        openMonitorInTheSameTab(i, ffi, pi, updateCursorPos: !isMulti);
      }
    }
  }
}
