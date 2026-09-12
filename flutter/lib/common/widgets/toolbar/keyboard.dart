part of 'toolbar.dart';

List<TToggleMenu> toolbarKeyboardToggles(FFI ffi) {
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final sessionId = ffi.sessionId;
  final isDefaultConn = ffi.connType == ConnType.defaultConn;
  List<TToggleMenu> v = [];

  // swap key
  if (ffiModel.keyboard &&
      ((isMacOS && pi.platform != kPeerPlatformMacOS) ||
          (!isMacOS && pi.platform == kPeerPlatformMacOS))) {
    final option = 'allow_swap_key';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    onChanged(bool? value) {
      if (value == null) return;
      bind.sessionToggleOption(sessionId: sessionId, value: option);
    }

    final enabled = !ffi.ffiModel.viewOnly;
    v.add(TToggleMenu(
        value: value,
        onChanged: enabled ? onChanged : null,
        child: Text(translate('Swap control-command key'))));
  }

  // Relative mouse mode (gaming mode).
  // Only show when server supports MOUSE_TYPE_MOVE_RELATIVE (version >= 1.4.5)
  // Note: This feature is only available in Flutter client. Sciter client does not support this.
  // Web client is not supported yet due to Pointer Lock API integration complexity with Flutter's input system.
  // Wayland is not supported due to cursor warping limitations.
  // Mobile: This option is now in GestureHelp widget, shown only when joystick is visible.
  final isWayland = isDesktop && isLinux && bind.mainCurrentIsWayland();
  if (isDesktop &&
      isDefaultConn &&
      !isWeb &&
      !isWayland &&
      ffiModel.keyboard &&
      !ffiModel.viewOnly &&
      ffi.inputModel.isRelativeMouseModeSupported) {
    v.add(TToggleMenu(
        value: ffi.inputModel.relativeMouseMode.value,
        onChanged: (value) {
          if (value == null) return;
          final previousValue = ffi.inputModel.relativeMouseMode.value;
          final success = ffi.inputModel.setRelativeMouseMode(value);
          if (!success) {
            // Revert the observable toggle to reflect the actual state
            ffi.inputModel.relativeMouseMode.value = previousValue;
          }
        },
        child: Text(translate('Relative mouse mode'))));
  }

  // reverse mouse wheel
  if (ffiModel.keyboard) {
    var optionValue =
        bind.sessionGetReverseMouseWheelSync(sessionId: sessionId) ?? '';
    if (optionValue == '') {
      optionValue = bind.mainGetUserDefaultOption(key: kKeyReverseMouseWheel);
    }
    onChanged(bool? value) async {
      if (value == null) return;
      await bind.sessionSetReverseMouseWheel(
          sessionId: sessionId, value: value ? 'Y' : 'N');
    }

    final enabled = !ffi.ffiModel.viewOnly;
    v.add(TToggleMenu(
        value: optionValue == 'Y',
        onChanged: enabled ? onChanged : null,
        child: Text(translate('Reverse mouse wheel'))));
  }

  // swap left right mouse
  if (ffiModel.keyboard) {
    final option = 'swap-left-right-mouse';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    onChanged(bool? value) {
      if (value == null) return;
      bind.sessionToggleOption(sessionId: sessionId, value: option);
    }

    final enabled = !ffi.ffiModel.viewOnly;
    v.add(TToggleMenu(
        value: value,
        onChanged: enabled ? onChanged : null,
        child: Text(translate('swap-left-right-mouse'))));
  }
  return v;
}
