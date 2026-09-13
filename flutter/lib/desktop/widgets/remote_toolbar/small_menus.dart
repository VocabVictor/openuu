part of 'remote_toolbar.dart';

class _PinMenu extends StatelessWidget {
  final ToolbarState state;
  const _PinMenu({Key? key, required this.state}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Obx(
      () => _IconMenuButton(
        assetName: state.pin ? "assets/pinned.svg" : "assets/unpinned.svg",
        tooltip: state.pin ? 'Unpin Toolbar' : 'Pin Toolbar',
        onPressed: state.switchPin,
        color:
            state.pin ? _ToolbarTheme.activeColor : _ToolbarTheme.blueColor,
        hoverColor: state.pin
            ? _ToolbarTheme.hoverActiveColor
            : _ToolbarTheme.hoverBlueColor,
      ),
    );
  }
}

class _MobileActionMenu extends StatelessWidget {
  final FFI ffi;
  const _MobileActionMenu({Key? key, required this.ffi}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    if (!ffi.ffiModel.isPeerAndroid) return Offstage();
    return Obx(() => _IconMenuButton(
          assetName: 'assets/actions_mobile.svg',
          tooltip: 'Mobile Actions',
          onPressed: () => ffi.dialogManager.setMobileActionsOverlayVisible(
              !ffi.dialogManager.mobileActionsOverlayVisible.value),
          color: ffi.dialogManager.mobileActionsOverlayVisible.isTrue
              ? _ToolbarTheme.activeColor
              : _ToolbarTheme.blueColor,
          hoverColor: ffi.dialogManager.mobileActionsOverlayVisible.isTrue
              ? _ToolbarTheme.hoverActiveColor
              : _ToolbarTheme.hoverBlueColor,
        ));
  }
}

class _MainMonitorSwitchButton extends StatelessWidget {
  final String id;
  final FFI ffi;

  const _MainMonitorSwitchButton({
    Key? key,
    required this.id,
    required this.ffi,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final cycle = _MonitorCycle(id, ffi);
    return Obx(() {
      if (cycle.total < 2) return const Offstage();
      final label = cycle.label;

      return _IconMenuButton(
        tooltip: cycle.tooltip,
        color: _ToolbarTheme.blueColor,
        hoverColor: _ToolbarTheme.hoverBlueColor,
        onPressed: cycle.next,
        icon: SizedBox(
          width: _ToolbarTheme.buttonSize,
          height: _ToolbarTheme.buttonSize,
          child: Stack(
            alignment: const Alignment(0, -0.125),
            children: [
              SvgPicture.asset(
                'assets/display_switcher.svg',
                colorFilter: ColorFilter.mode(
                    _ToolbarTheme.iconColor(_ToolbarTheme.blueColor,
                        dark: Theme.of(context).brightness == Brightness.dark),
                    BlendMode.srcIn),
                width: _ToolbarTheme.buttonSize,
                height: _ToolbarTheme.buttonSize,
              ),
              Text(
                label,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  color: UiColor.text,
                  fontSize: 10,
                  height: 1,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ],
          ),
        ),
      );
    });
  }
}

class _ControlMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  final ToolbarState state;
  _ControlMenu(
      {Key? key, required this.id, required this.ffi, required this.state})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _IconSubmenuButton(
        tooltip: 'Control Actions',
        svg: "assets/actions.svg",
        color: _ToolbarTheme.blueColor,
        hoverColor: _ToolbarTheme.hoverBlueColor,
        ffi: ffi,
        menuChildrenGetter: (_) => toolbarControls(context, id, ffi).map((e) {
              if (e.divider) {
                return Divider();
              } else {
                return MenuButton(
                    child: e.child,
                    onPressed: e.onPressed,
                    ffi: ffi,
                    trailingIcon: e.trailingIcon);
              }
            }).toList());
  }
}

class _RecordMenu extends StatelessWidget {
  const _RecordMenu({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    var ffi = Provider.of<FfiModel>(context);
    var recordingModel = Provider.of<RecordingModel>(context);
    final hideRecordingButton =
        bind.mainGetLocalOption(key: kOptionHideRecordingButton) == 'Y';
    final visible = !hideRecordingButton &&
        (recordingModel.start || ffi.permissions['recording'] != false);
    if (!visible) return Offstage();
    return _IconMenuButton(
      assetName: 'assets/rec.svg',
      tooltip: recordingModel.start
          ? 'Stop session recording'
          : 'Start session recording',
      onPressed: () => recordingModel.toggle(),
      color: recordingModel.start
          ? _ToolbarTheme.redColor
          : _ToolbarTheme.blueColor,
      hoverColor: recordingModel.start
          ? _ToolbarTheme.hoverRedColor
          : _ToolbarTheme.hoverBlueColor,
    );
  }
}

class _CloseMenu extends StatelessWidget {
  final String id;
  final FFI ffi;
  const _CloseMenu({Key? key, required this.id, required this.ffi})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return _IconMenuButton(
      assetName: 'assets/close.svg',
      tooltip: 'Close',
      onPressed: () async {
        if (await showConnEndAuditDialogCloseCanceled(ffi: ffi)) {
          return;
        }
        closeConnection(id: id);
      },
      color: _ToolbarTheme.redColor,
      hoverColor: _ToolbarTheme.hoverRedColor,
    );
  }
}

class _MinimizedMonitorSwitchButton extends StatelessWidget {
  final String id;
  final FFI ffi;

  const _MinimizedMonitorSwitchButton({
    Key? key,
    required this.id,
    required this.ffi,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    const double iconSize = 20;
    final cycle = _MonitorCycle(id, ffi);

    return Obx(() {
      final label = cycle.label;
      if (!mainGetLocalBoolOptionSync(kOptionAllowMonitorSwitchMainToolbar) ||
          !mainGetLocalBoolOptionSync(kOptionAllowMonitorSwitchMinToolbar)) {
        return const Offstage();
      }
      if (cycle.total < 2) return const Offstage();
      final privacyModeState = PrivacyModeState.find(id);
      if (privacyModeState.isNotEmpty &&
          !allowDisplaySwitchInPrivacyMode(
              ffi.ffiModel.pi, privacyModeState.value)) {
        return const Offstage();
      }

      return Tooltip(
        message: cycle.tooltip,
        child: TextButton(
          onPressed: cycle.next,
          style: ButtonStyle(
            minimumSize: MaterialStateProperty.all(const Size(0, 0)),
            padding: MaterialStateProperty.all(EdgeInsets.zero),
            backgroundColor: MaterialStateProperty.resolveWith((states) {
              if (states.contains(MaterialState.hovered)) {
                return _ToolbarTheme.hoverBlueColor;
              }
              return null;
            }),
          ),
          child: Stack(
            alignment: const Alignment(0, -0.125),
            children: [
              SvgPicture.asset(
                'assets/display_switcher.svg',
                colorFilter: ColorFilter.mode(
                    _ToolbarTheme.iconColor(_ToolbarTheme.blueColor,
                        dark: Theme.of(context).brightness == Brightness.dark),
                    BlendMode.srcIn),
                width: iconSize,
                height: iconSize,
              ),
              Text(
                label,
                style: const TextStyle(
                  color: UiColor.text,
                  fontSize: 9,
                  height: 1,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ],
          ),
        ),
      );
    });
  }
}
