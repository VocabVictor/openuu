part of 'desktop_setting_page.dart';

extension _DisplayQuality on _DisplayState {
  Widget imageQuality(BuildContext context) {
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(
          key: kOptionImageQuality, value: value);
      _setState(() {});
    }

    final isOptFixed = isOptionFixed(kOptionImageQuality);
    final groupValue = bind.mainGetUserDefaultOption(key: kOptionImageQuality);
    if (isWindows && !bind.isIncomingOnly()) {
      return _choiceCard(context, 'Default Image Quality', groupValue, {
        kRemoteImageQualityBest: 'Good image quality',
        kRemoteImageQualityBalanced: 'Balanced',
        kRemoteImageQualityLow: 'Optimize reaction time',
        kRemoteImageQualityCustom: 'Custom',
      }, onChanged, enabled: !isOptFixed,
        detail: groupValue == kRemoteImageQualityCustom
          ? customImageQualitySetting() : null);
    }
    return _Card(title: 'Default Image Quality', children: [
      _Radio(context,
          value: kRemoteImageQualityBest,
          groupValue: groupValue,
          label: 'Good image quality',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: kRemoteImageQualityBalanced,
          groupValue: groupValue,
          label: 'Balanced',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: kRemoteImageQualityLow,
          groupValue: groupValue,
          label: 'Optimize reaction time',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: kRemoteImageQualityCustom,
          groupValue: groupValue,
          label: 'Custom',
          onChanged: isOptFixed ? null : onChanged),
      Offstage(
        offstage: groupValue != kRemoteImageQualityCustom,
        child: customImageQualitySetting(),
      )
    ]);
  }

  Widget trackpadSpeed(BuildContext context) {
    final initSpeed =
        (int.tryParse(bind.mainGetUserDefaultOption(key: kKeyTrackpadSpeed)) ??
            kDefaultTrackpadSpeed);
    final curSpeed = SimpleWrapper(initSpeed);
    void onDebouncer(int v) {
      bind.mainSetUserDefaultOption(
          key: kKeyTrackpadSpeed, value: v.toString());
      // It's better to notify all sessions that the default speed is changed.
      // But it may also be ok to take effect in the next connection.
    }

    if (isWindows && !bind.isIncomingOnly()) {
      return _group(null, [
        _settingRow(
            context,
            'Default trackpad speed',
            TrackpadSpeedWidget(
                value: curSpeed, onDebouncer: onDebouncer, compact: true)),
      ]);
    }
    return _Card(title: 'Default trackpad speed', children: [
      TrackpadSpeedWidget(
        value: curSpeed,
        onDebouncer: onDebouncer,
      ),
    ]);
  }

  Widget codec(BuildContext context) {
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(
          key: kOptionCodecPreference, value: value);
      _setState(() {});
    }

    final groupValue =
        bind.mainGetUserDefaultOption(key: kOptionCodecPreference);
    var hwRadios = [];
    final codecOptions = {'auto': 'Auto', 'vp8': 'VP8', 'vp9': 'VP9', 'av1': 'AV1'};
    final isOptFixed = isOptionFixed(kOptionCodecPreference);
    try {
      final Map codecsJson = jsonDecode(bind.mainSupportedHwdecodings());
      final h264 = codecsJson['h264'] ?? false;
      final h265 = codecsJson['h265'] ?? false;
      if (h264) {
        codecOptions['h264'] = 'H264';
        hwRadios.add(_Radio(context,
            value: 'h264',
            groupValue: groupValue,
            label: 'H264',
            onChanged: isOptFixed ? null : onChanged));
      }
      if (h265) {
        codecOptions['h265'] = 'H265';
        hwRadios.add(_Radio(context,
            value: 'h265',
            groupValue: groupValue,
            label: 'H265',
            onChanged: isOptFixed ? null : onChanged));
      }
    } catch (e) {
      debugPrint("failed to parse supported hwdecodings, err=$e");
    }
    if (isWindows && !bind.isIncomingOnly()) {
      return _choiceCard(context, 'Default Codec', groupValue, codecOptions,
        onChanged, enabled: !isOptFixed, width: 240);
    }
    return _Card(title: 'Default Codec', children: [
      _Radio(context,
          value: 'auto',
          groupValue: groupValue,
          label: 'Auto',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: 'vp8',
          groupValue: groupValue,
          label: 'VP8',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: 'vp9',
          groupValue: groupValue,
          label: 'VP9',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: 'av1',
          groupValue: groupValue,
          label: 'AV1',
          onChanged: isOptFixed ? null : onChanged),
      ...hwRadios,
    ]);
  }
}
