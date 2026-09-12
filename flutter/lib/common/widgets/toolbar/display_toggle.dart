part of 'toolbar.dart';

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
