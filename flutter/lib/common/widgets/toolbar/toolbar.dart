import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:flutter_hbb/models/input_model.dart';
part 'wayland.dart';
part 'controls.dart';
part 'display.dart';
part 'display_toggle.dart';
part 'keyboard.dart';
part 'virtual_display.dart';

bool isEditOsPassword = false;
const String kPeerOptionAllowWaylandKeyboard = 'allow-wayland-keyboard';
const String kWaylandKeyboardIssueUrl =
    'https://github.com/rustdesk/rustdesk/issues/14586';

bool _isWindowsMode1PrivacyImpl(String privacyModeImpl) {
  return privacyModeImpl == kPrivacyModeImplMag ||
      privacyModeImpl == kPrivacyModeImplExcludeFromCapture;
}

// macOS privacy mode blacks out all online displays. Windows Mode 1 also
// covers every local monitor with privacy overlay windows, so remote display
// switching does not weaken local privacy protection.
//
// Keep this separate from the capture backend capability. The legacy Windows
// magnifier capturer is not reliable for multi-monitor capture; WebRTC's
// screen_capturer_win_magnifier also disables it when SM_CMONITORS != 1:
// https://webrtc.googlesource.com/src/+/1845922d5a1bf9c27deeffb4a8c8daea124434c1/modules/desktop_capture/win/screen_capturer_win_magnifier.cc
bool allowDisplaySwitchInPrivacyMode(PeerInfo pi, String privacyModeImpl) {
  return pi.platform == kPeerPlatformMacOS ||
      (pi.platform == kPeerPlatformWindows &&
          _isWindowsMode1PrivacyImpl(privacyModeImpl) &&
          versionCmp(pi.version, '1.4.8') >= 0);
}

class TTextMenu {
  final Widget child;
  final VoidCallback? onPressed;
  Widget? trailingIcon;
  bool divider;
  TTextMenu(
      {required this.child,
      required this.onPressed,
      this.trailingIcon,
      this.divider = false});

  Widget getChild() {
    if (trailingIcon != null) {
      return Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          child,
          trailingIcon!,
        ],
      );
    } else {
      return child;
    }
  }
}

class TRadioMenu<T> {
  final Widget child;
  final T value;
  final T groupValue;
  final ValueChanged<T?>? onChanged;

  TRadioMenu(
      {required this.child,
      required this.value,
      required this.groupValue,
      required this.onChanged});
}

class TToggleMenu {
  final Widget child;
  final bool value;
  final ValueChanged<bool?>? onChanged;
  TToggleMenu(
      {required this.child, required this.value, required this.onChanged});
}

handleOsPasswordEditIcon(
    SessionID sessionId, OverlayDialogManager dialogManager) {
  isEditOsPassword = true;
  showSetOSPassword(
      sessionId, false, dialogManager, null, () => isEditOsPassword = false);
}

handleOsPasswordAction(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  if (isEditOsPassword) {
    isEditOsPassword = false;
    return;
  }
  final password =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-password') ??
          '';
  if (password.isEmpty) {
    showSetOSPassword(sessionId, true, dialogManager, password,
        () => isEditOsPassword = false);
  } else {
    bind.sessionInputOsPassword(sessionId: sessionId, value: password);
  }
}
