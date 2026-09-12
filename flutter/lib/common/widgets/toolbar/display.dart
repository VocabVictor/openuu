part of 'toolbar.dart';

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
