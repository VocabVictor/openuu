part of 'desktop_setting_page.dart';

extension _GeneralDesktop on _GeneralState {
  /// The service as a status bar above the groups: dot, state, Start/Stop.
  Widget _serviceBar(BuildContext context, VoidCallback onToggle) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final stopped = serviceStop.value;
    return Padding(
        padding: const EdgeInsets.only(top: UiSpace.settingsGroupGap),
        child: Container(
            height: UiSpace.settingsStatusBarHeight,
            padding: const EdgeInsets.symmetric(
                horizontal: UiSpace.settingsRowPaddingX),
            decoration: BoxDecoration(
                color: stopped
                    ? UiColor.of(context).statusStoppedBg
                    : UiColor.of(context).statusRunningBg,
                borderRadius:
                    BorderRadius.circular(UiSpace.settingsGroupRadius)),
            child: Row(children: [
              Container(
                  width: UiSpace.statusDotSize,
                  height: UiSpace.statusDotSize,
                  decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: stopped ? UiColor.of(context).favorite : UiColor.of(context).ready)),
              const SizedBox(width: UiSpace.s2),
              Text(
                  stopped
                      ? (zh ? '服务已停止' : 'Service stopped')
                      : (zh ? '服务运行中' : 'Service running'),
                  style:
                      UiType.of(context).rowTitle.copyWith(fontWeight: FontWeight.w400)),
              const Spacer(),
              _secondaryButton(stopped ? 'Start' : 'Stop',
                  serviceBtnEnabled.value ? onToggle : null),
            ])));
  }

  /// Hardware codec and audio input as one group on the desktop shell.
  Widget _mediaGroup(BuildContext context, bool zh) {
    final hasHwcodec = bind.mainHasHwcodec() || bind.mainHasVram();
    final outgoingOnly = bind.isOutgoingOnly();
    if (!hasHwcodec && outgoingOnly) return const Offstage();
    return _group(zh ? '音视频' : 'Audio and video', [
      if (hasHwcodec)
        _OptionCheckBox(context, 'Enable hardware codec', kOptionEnableHwcodec,
            update: (bool v) {
          if (v) {
            bind.mainCheckHwcodec();
          }
        }),
      if (!outgoingOnly)
        AudioInput(
            builder: (devices, currentDevice, setDevice) => _settingRow(
                context,
                'Audio Input Device',
                SettingsDropdown(
                  keys: devices,
                  values: devices,
                  current: currentDevice,
                  width: 240,
                  onChanged: (key) async {
                    setDevice(key);
                    _setState(() {});
                  },
                )),
            isCm: false,
            isVoiceCall: false),
    ]);
  }

}
