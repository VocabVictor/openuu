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

Future<List<TRadioMenu<String>>> toolbarViewStyle(
    BuildContext context, String id, FFI ffi) async {
  final groupValue =
      await bind.sessionGetViewStyle(sessionId: ffi.sessionId) ?? '';
  void onChanged(String? value) async {
    if (value == null) return;
    bind
        .sessionSetViewStyle(sessionId: ffi.sessionId, value: value)
        .then((_) => ffi.canvasModel.updateViewStyle());
  }

  return [
    TRadioMenu<String>(
        child: Text(translate('Scale original')),
        value: kRemoteViewStyleOriginal,
        groupValue: groupValue,
        onChanged: onChanged),
    TRadioMenu<String>(
        child: Text(translate('Scale adaptive')),
        value: kRemoteViewStyleAdaptive,
        groupValue: groupValue,
        onChanged: onChanged),
    TRadioMenu<String>(
        child: Text(translate('Scale custom')),
        value: kRemoteViewStyleCustom,
        groupValue: groupValue,
        onChanged: onChanged)
  ];
}

Future<List<TRadioMenu<String>>> toolbarImageQuality(
    BuildContext context, String id, FFI ffi) async {
  final groupValue =
      await bind.sessionGetImageQuality(sessionId: ffi.sessionId) ?? '';
  onChanged(String? value) async {
    if (value == null) return;
    await bind.sessionSetImageQuality(sessionId: ffi.sessionId, value: value);
  }

  return [
    TRadioMenu<String>(
        child: Text(translate('Good image quality')),
        value: kRemoteImageQualityBest,
        groupValue: groupValue,
        onChanged: onChanged),
    TRadioMenu<String>(
        child: Text(translate('Balanced')),
        value: kRemoteImageQualityBalanced,
        groupValue: groupValue,
        onChanged: onChanged),
    TRadioMenu<String>(
        child: Text(translate('Optimize reaction time')),
        value: kRemoteImageQualityLow,
        groupValue: groupValue,
        onChanged: onChanged),
    TRadioMenu<String>(
      child: Text(translate('Custom')),
      value: kRemoteImageQualityCustom,
      groupValue: groupValue,
      onChanged: (value) {
        onChanged(value);
        customImageQualityDialog(ffi.sessionId, id, ffi);
      },
    ),
  ];
}

Future<List<TRadioMenu<String>>> toolbarCodec(
    BuildContext context, String id, FFI ffi) async {
  final sessionId = ffi.sessionId;
  final alternativeCodecs =
      await bind.sessionAlternativeCodecs(sessionId: sessionId);
  final groupValue = await bind.sessionGetOption(
          sessionId: sessionId, arg: kOptionCodecPreference) ??
      '';
  final List<bool> codecs = [];
  try {
    final Map codecsJson = jsonDecode(alternativeCodecs);
    final vp8 = codecsJson['vp8'] ?? false;
    final av1 = codecsJson['av1'] ?? false;
    final h264 = codecsJson['h264'] ?? false;
    final h265 = codecsJson['h265'] ?? false;
    codecs.add(vp8);
    codecs.add(av1);
    codecs.add(h264);
    codecs.add(h265);
  } catch (e) {
    debugPrint("Show Codec Preference err=$e");
  }
  final visible =
      codecs.length == 4 && (codecs[0] || codecs[1] || codecs[2] || codecs[3]);
  if (!visible) return [];
  onChanged(String? value) async {
    if (value == null) return;
    await bind.sessionPeerOption(
        sessionId: sessionId, name: kOptionCodecPreference, value: value);
    bind.sessionChangePreferCodec(sessionId: sessionId);
  }

  TRadioMenu<String> radio(String label, String value, bool enabled) {
    return TRadioMenu<String>(
        child: Text(label),
        value: value,
        groupValue: groupValue,
        onChanged: enabled ? onChanged : null);
  }

  var autoLabel = translate('Auto');
  if (groupValue == 'auto' &&
      ffi.qualityMonitorModel.data.codecFormat != null) {
    autoLabel = '$autoLabel (${ffi.qualityMonitorModel.data.codecFormat})';
  }
  return [
    radio(autoLabel, 'auto', true),
    if (codecs[0]) radio('VP8', 'vp8', codecs[0]),
    radio('VP9', 'vp9', true),
    if (codecs[1]) radio('AV1', 'av1', codecs[1]),
    if (codecs[2]) radio('H264', 'h264', codecs[2]),
    if (codecs[3]) radio('H265', 'h265', codecs[3]),
  ];
}

Future<List<TToggleMenu>> toolbarCursor(
    BuildContext context, String id, FFI ffi) async {
  List<TToggleMenu> v = [];
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final sessionId = ffi.sessionId;

  // show remote cursor
  if (pi.platform != kPeerPlatformAndroid &&
      !ffi.canvasModel.cursorEmbedded &&
      !pi.isWayland) {
    final state = ShowRemoteCursorState.find(id);
    final lockState = ShowRemoteCursorLockState.find(id);
    final enabled = !ffiModel.viewOnly;
    final option = 'show-remote-cursor';
    if (pi.currentDisplay == kAllDisplayValue ||
        bind.sessionIsMultiUiSession(sessionId: sessionId)) {
      lockState.value = false;
    }
    v.add(TToggleMenu(
        child: Text(translate('Show remote cursor')),
        value: state.value,
        onChanged: enabled && !lockState.value
            ? (value) async {
                if (value == null) return;
                await bind.sessionToggleOption(
                    sessionId: sessionId, value: option);
                state.value = bind.sessionGetToggleOptionSync(
                    sessionId: sessionId, arg: option);
              }
            : null));
  }
  // follow remote cursor
  if (pi.platform != kPeerPlatformAndroid &&
      !ffi.canvasModel.cursorEmbedded &&
      !pi.isWayland &&
      versionCmp(pi.version, "1.2.4") >= 0 &&
      pi.displays.length > 1 &&
      pi.currentDisplay != kAllDisplayValue &&
      !bind.sessionIsMultiUiSession(sessionId: sessionId)) {
    final option = 'follow-remote-cursor';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    final showCursorOption = 'show-remote-cursor';
    final showCursorState = ShowRemoteCursorState.find(id);
    final showCursorLockState = ShowRemoteCursorLockState.find(id);
    final showCursorEnabled = bind.sessionGetToggleOptionSync(
        sessionId: sessionId, arg: showCursorOption);
    showCursorLockState.value = value;
    if (value && !showCursorEnabled) {
      await bind.sessionToggleOption(
          sessionId: sessionId, value: showCursorOption);
      showCursorState.value = bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: showCursorOption);
    }
    v.add(TToggleMenu(
        child: Text(translate('Follow remote cursor')),
        value: value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(sessionId: sessionId, value: option);
          value = bind.sessionGetToggleOptionSync(
              sessionId: sessionId, arg: option);
          showCursorLockState.value = value;
          if (!showCursorEnabled) {
            await bind.sessionToggleOption(
                sessionId: sessionId, value: showCursorOption);
            showCursorState.value = bind.sessionGetToggleOptionSync(
                sessionId: sessionId, arg: showCursorOption);
          }
        }));
  }
  // follow remote window focus
  if (pi.platform != kPeerPlatformAndroid &&
      !ffi.canvasModel.cursorEmbedded &&
      !pi.isWayland &&
      versionCmp(pi.version, "1.2.4") >= 0 &&
      pi.displays.length > 1 &&
      pi.currentDisplay != kAllDisplayValue &&
      !bind.sessionIsMultiUiSession(sessionId: sessionId)) {
    final option = 'follow-remote-window';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        child: Text(translate('Follow remote window focus')),
        value: value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(sessionId: sessionId, value: option);
          value = bind.sessionGetToggleOptionSync(
              sessionId: sessionId, arg: option);
        }));
  }
  // zoom cursor
  final viewStyle = await bind.sessionGetViewStyle(sessionId: sessionId) ?? '';
  if (!isMobile &&
      pi.platform != kPeerPlatformAndroid &&
      viewStyle != kRemoteViewStyleOriginal) {
    final option = 'zoom-cursor';
    final peerState = PeerBoolOption.find(id, option);
    v.add(TToggleMenu(
      child: Text(translate('Zoom cursor')),
      value: peerState.value,
      onChanged: (value) async {
        if (value == null) return;
        await bind.sessionToggleOption(sessionId: sessionId, value: option);
        peerState.value =
            bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
      },
    ));
  }
  return v;
}

Future<List<TToggleMenu>> toolbarDisplayToggle(
    BuildContext context, String id, FFI ffi) async {
  List<TToggleMenu> v = [];
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final perms = ffiModel.permissions;
  final sessionId = ffi.sessionId;
  final isDefaultConn = ffi.connType == ConnType.defaultConn;

  // show quality monitor
  final option = 'show-quality-monitor';
  v.add(TToggleMenu(
      value: bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option),
      onChanged: (value) async {
        if (value == null) return;
        await bind.sessionToggleOption(sessionId: sessionId, value: option);
        ffi.qualityMonitorModel.checkShowQualityMonitor(sessionId);
      },
      child: Text(translate('Show quality monitor'))));
  // mute
  if (isDefaultConn && perms['audio'] != false) {
    final option = 'disable-audio';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(sessionId: sessionId, value: option);
        },
        child: Text(translate('Mute'))));
  }
  // file copy and paste
  // If the version is less than 1.2.4, file copy and paste is supported on Windows only.
  final isSupportIfPeer_1_2_3 = versionCmp(pi.version, '1.2.4') < 0 &&
      isWindows &&
      pi.platform == kPeerPlatformWindows;
  // If the version is 1.2.4 or later, file copy and paste is supported when kPlatformAdditionsHasFileClipboard is set.
  final isSupportIfPeer_1_2_4 = versionCmp(pi.version, '1.2.4') >= 0 &&
      bind.mainHasFileClipboard() &&
      pi.platformAdditions.containsKey(kPlatformAdditionsHasFileClipboard);
  if (isDefaultConn &&
      ffiModel.keyboard &&
      perms['file'] != false &&
      (isSupportIfPeer_1_2_3 || isSupportIfPeer_1_2_4)) {
    final enabled = !ffiModel.viewOnly;
    final value = bind.sessionGetToggleOptionSync(
        sessionId: sessionId, arg: kOptionEnableFileCopyPaste);
    v.add(TToggleMenu(
        value: value,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                bind.sessionToggleOption(
                    sessionId: sessionId, value: kOptionEnableFileCopyPaste);
              }
            : null,
        child: Text(translate('Enable file copy and paste'))));
  }
  // disable clipboard
  if (isDefaultConn && ffiModel.keyboard && perms['clipboard'] != false) {
    final enabled = !ffiModel.viewOnly;
    final option = 'disable-clipboard';
    var value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    if (ffiModel.viewOnly) value = true;
    v.add(TToggleMenu(
        value: value,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                bind.sessionToggleOption(sessionId: sessionId, value: option);
              }
            : null,
        child: Text(translate('Disable clipboard'))));
  }
  // lock after session end
  if (isDefaultConn && ffiModel.keyboard && !ffiModel.isPeerAndroid) {
    final enabled = !ffiModel.viewOnly;
    final option = 'lock-after-session-end';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        value: value,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                bind.sessionToggleOption(sessionId: sessionId, value: option);
              }
            : null,
        child: Text(translate('Lock after session end'))));
  }

  final privacyModeState = PrivacyModeState.find(id);
  if (pi.isSupportMultiDisplay &&
      (privacyModeState.isEmpty ||
          allowDisplaySwitchInPrivacyMode(pi, privacyModeState.value)) &&
      pi.displaysCount.value > 1 &&
      bind.mainGetUserDefaultOption(key: kKeyShowMonitorsToolbar) == 'Y') {
    final value =
        bind.sessionGetDisplaysAsIndividualWindows(sessionId: ffi.sessionId) ==
            'Y';
    v.add(TToggleMenu(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionSetDisplaysAsIndividualWindows(
              sessionId: sessionId, value: value ? 'Y' : 'N');
        },
        child: Text(translate('Show displays as individual windows'))));
  }

  final isMultiScreens = !isWeb && (await getScreenRectList()).length > 1;
  if (pi.isSupportMultiDisplay && isMultiScreens) {
    final value = bind.sessionGetUseAllMyDisplaysForTheRemoteSession(
            sessionId: ffi.sessionId) ==
        'Y';
    v.add(TToggleMenu(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionSetUseAllMyDisplaysForTheRemoteSession(
              sessionId: sessionId, value: value ? 'Y' : 'N');
        },
        child: Text(translate('Use all my displays for the remote session'))));
  }

  // 444
  final codec_format = ffi.qualityMonitorModel.data.codecFormat;
  if (versionCmp(pi.version, "1.2.4") >= 0 &&
      (codec_format == "AV1" || codec_format == "VP9")) {
    final option = 'i444';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        value: value,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(sessionId: sessionId, value: option);
          bind.sessionChangePreferCodec(sessionId: sessionId);
        },
        child: Text(translate('True color (4:4:4)'))));
  }

  if (isDefaultConn && isMobile) {
    v.addAll(toolbarKeyboardToggles(ffi));
  }

  // view mode (mobile only, desktop is in keyboard menu)
  if (isDefaultConn && isMobile && versionCmp(pi.version, '1.2.0') >= 0) {
    v.add(TToggleMenu(
        value: ffiModel.viewOnly,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(
              sessionId: ffi.sessionId, value: kOptionToggleViewOnly);
          ffiModel.setViewOnly(id, value);
        },
        child: Text(translate('View Mode'))));
  }
  return v;
}

var togglePrivacyModeTime = DateTime.now().subtract(const Duration(hours: 1));

List<TToggleMenu> toolbarPrivacyMode(
    RxString privacyModeState, BuildContext context, String id, FFI ffi) {
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final sessionId = ffi.sessionId;
  final hasPrivacyModePermission =
      ffiModel.permissions['privacy_mode'] != false;

  // Backend revocation already attempts to turn privacy mode off.
  // Still keep this menu when privacy mode is active, so users can turn it off
  // if there is a sync delay, version mismatch, or off attempt failure.
  if (!hasPrivacyModePermission && privacyModeState.isEmpty) {
    return []; // No permission and not active, hide options.
  }

  bool checkDisplayAllowedForPrivacyMode(String targetImplKey, bool turnOn) {
    if (!turnOn ||
        allowDisplaySwitchInPrivacyMode(pi, targetImplKey) ||
        (ffiModel.pi.currentDisplay == 0 &&
            !bind.sessionIsMultiUiSession(sessionId: sessionId))) {
      return true;
    }
    msgBox(sessionId, 'custom-nook-nocancel-hasclose', 'info',
        'Please switch to Display 1 first', '', ffi.dialogManager);
    return false;
  }

  getDefaultMenu(Future<void> Function(SessionID sid, String opt) toggleFunc,
      String targetImplKey) {
    final enabled = !ffiModel.viewOnly &&
        (hasPrivacyModePermission || privacyModeState.isNotEmpty);
    return TToggleMenu(
        value: privacyModeState.isNotEmpty,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                if (!checkDisplayAllowedForPrivacyMode(targetImplKey, value)) {
                  return;
                }
                final option = 'privacy-mode';
                toggleFunc(sessionId, option);
              }
            : null,
        child: Text(translate('Privacy mode')));
  }

  final privacyModeImpls =
      pi.platformAdditions[kPlatformAdditionsSupportedPrivacyModeImpl]
          as List<dynamic>?;
  if (privacyModeImpls == null) {
    return [
      getDefaultMenu((sid, opt) async {
        bind.sessionToggleOption(sessionId: sid, value: opt);
        togglePrivacyModeTime = DateTime.now();
      }, kPrivacyModeImplMag)
    ];
  }
  if (privacyModeImpls.isEmpty) {
    return [];
  }

  if (privacyModeImpls.length == 1) {
    final implKey = (privacyModeImpls[0] as List<dynamic>)[0] as String;
    return [
      getDefaultMenu((sid, opt) async {
        bind.sessionTogglePrivacyMode(
            sessionId: sid, implKey: implKey, on: privacyModeState.isEmpty);
        togglePrivacyModeTime = DateTime.now();
      }, implKey)
    ];
  } else {
    final visibleImpls = hasPrivacyModePermission
        ? privacyModeImpls
        : privacyModeImpls.where((e) {
            final implKey = (e as List<dynamic>)[0] as String;
            return privacyModeState.value == implKey;
          }).toList();
    return visibleImpls.map((e) {
      final implKey = (e as List<dynamic>)[0] as String;
      final implName = (e)[1] as String;
      final enabled = !ffiModel.viewOnly &&
          (hasPrivacyModePermission || privacyModeState.value == implKey);
      return TToggleMenu(
          child: Text(translate(implName)),
          value: privacyModeState.value == implKey,
          onChanged: enabled
              ? (value) {
                  if (value == null) return;
                  if (value && !hasPrivacyModePermission) return;
                  if (!checkDisplayAllowedForPrivacyMode(implKey, value)) {
                    return;
                  }
                  togglePrivacyModeTime = DateTime.now();
                  bind.sessionTogglePrivacyMode(
                      sessionId: sessionId, implKey: implKey, on: value);
                }
              : null);
    }).toList();
  }
}

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

bool showVirtualDisplayMenu(FFI ffi) {
  if (ffi.ffiModel.pi.platform != kPeerPlatformWindows) {
    return false;
  }
  if (!ffi.ffiModel.pi.isInstalled) {
    return false;
  }
  if (ffi.ffiModel.pi.isRustDeskIdd || ffi.ffiModel.pi.isAmyuniIdd) {
    return true;
  }
  return false;
}

List<Widget> getVirtualDisplayMenuChildren(
    FFI ffi, String id, VoidCallback? clickCallBack) {
  if (!showVirtualDisplayMenu(ffi)) {
    return [];
  }
  final pi = ffi.ffiModel.pi;
  final privacyModeState = PrivacyModeState.find(id);
  if (pi.isRustDeskIdd) {
    final virtualDisplays = ffi.ffiModel.pi.RustDeskVirtualDisplays;
    final children = <Widget>[];
    for (var i = 0; i < kMaxVirtualDisplayCount; i++) {
      children.add(Obx(() => CkbMenuButton(
            value: virtualDisplays.contains(i + 1),
            onChanged: privacyModeState.isNotEmpty
                ? null
                : (bool? value) async {
                    if (value != null) {
                      bind.sessionToggleVirtualDisplay(
                          sessionId: ffi.sessionId, index: i + 1, on: value);
                      clickCallBack?.call();
                    }
                  },
            child: Text('${translate('Virtual display')} ${i + 1}'),
            ffi: ffi,
          )));
    }
    children.add(Divider());
    children.add(Obx(() => MenuButton(
          onPressed: privacyModeState.isNotEmpty
              ? null
              : () {
                  bind.sessionToggleVirtualDisplay(
                      sessionId: ffi.sessionId,
                      index: kAllVirtualDisplay,
                      on: false);
                  clickCallBack?.call();
                },
          ffi: ffi,
          child: Text(translate('Plug out all')),
        )));
    return children;
  }
  if (pi.isAmyuniIdd) {
    final count = ffi.ffiModel.pi.amyuniVirtualDisplayCount;
    final children = <Widget>[
      Obx(() => Row(
            children: [
              TextButton(
                onPressed: privacyModeState.isNotEmpty || count == 0
                    ? null
                    : () {
                        bind.sessionToggleVirtualDisplay(
                            sessionId: ffi.sessionId, index: 0, on: false);
                        clickCallBack?.call();
                      },
                child: Icon(Icons.remove),
              ),
              Text(count.toString()),
              TextButton(
                onPressed: privacyModeState.isNotEmpty || count == 4
                    ? null
                    : () {
                        bind.sessionToggleVirtualDisplay(
                            sessionId: ffi.sessionId, index: 0, on: true);
                        clickCallBack?.call();
                      },
                child: Icon(Icons.add),
              ),
            ],
          )),
      Divider(),
      Obx(() => MenuButton(
            onPressed: privacyModeState.isNotEmpty || count == 0
                ? null
                : () {
                    bind.sessionToggleVirtualDisplay(
                        sessionId: ffi.sessionId,
                        index: kAllVirtualDisplay,
                        on: false);
                    clickCallBack?.call();
                  },
            ffi: ffi,
            child: Text(translate('Plug out all')),
          )),
    ];
    return children;
  }
  return [];
}
