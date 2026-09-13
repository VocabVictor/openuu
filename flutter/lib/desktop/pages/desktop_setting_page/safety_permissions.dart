part of 'desktop_setting_page.dart';

extension _SafetyPermissions on _SafetyState {
  Widget permissions(context) {
    bool enabled = !locked;
    // Simple temp wrapper for PR check
    tmpWrapper() {
      String accessMode = bind.mainGetOptionSync(key: kOptionAccessMode);
      _AccessMode mode;
      if (accessMode == 'full') {
        mode = _AccessMode.full;
      } else if (accessMode == 'view') {
        mode = _AccessMode.view;
      } else {
        mode = _AccessMode.custom;
      }
      String initialKey;
      bool? fakeValue;
      switch (mode) {
        case _AccessMode.custom:
          initialKey = '';
          fakeValue = null;
          break;
        case _AccessMode.full:
          initialKey = 'full';
          fakeValue = true;
          break;
        case _AccessMode.view:
          initialKey = 'view';
          fakeValue = false;
          break;
      }

      final desktop = isWindows && !bind.isIncomingOnly();
      final presetKeys = <String>[defaultOptionAccessMode, 'full', 'view'];
      final presetValues = [
        translate('Custom'),
        translate('Full Access'),
        translate('Screen Share'),
      ];
      onPreset(String mode) async {
        await bind.mainSetOption(key: kOptionAccessMode, value: mode);
        _setState(() {});
      }
      final presetEnabled = enabled && !isOptionFixed(kOptionAccessMode);
      final preset = desktop
          ? SettingsDropdown(
              keys: presetKeys,
              values: presetValues,
              current: initialKey,
              enabled: presetEnabled,
              onChanged: onPreset)
          : ComboBox(
                  keys: presetKeys,
                  values: presetValues,
                  enabled: presetEnabled,
                  initialKey: initialKey,
                  onChanged: onPreset)
              .marginOnly(left: _kContentHMargin);
      return _Card(
          title: 'Permissions',
          title_suffix: desktop ? [preset] : null,
          children: [
        if (!desktop) preset,
        if (desktop && mode != _AccessMode.custom)
          Container(
              padding: const EdgeInsets.symmetric(
                  horizontal: UiSpace.settingsRowPaddingX,
                  vertical: UiSpace.s2),
              color: UiColor.primaryTint,
              child: Text(
                  Localizations.localeOf(context).languageCode == 'zh'
                      ? '当前使用「${presetValues[presetKeys.indexOf(initialKey)]}」预设，下列权限由预设决定；改为「自定义」后可单独调整。'
                      : 'The "${presetValues[presetKeys.indexOf(initialKey)]}" preset decides the permissions below; switch to "Custom" to adjust them one by one.',
                  style: UiType.caption.copyWith(color: UiColor.primary))),
        Column(
          children: [
            _OptionCheckBox(
                context, 'Enable keyboard/mouse', kOptionEnableKeyboard,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(context, 'Enable clipboard', kOptionEnableClipboard,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(
                context, 'Enable file transfer', kOptionEnableFileTransfer,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(context, 'Enable audio', kOptionEnableAudio,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(context, 'Enable camera', kOptionEnableCamera,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(context, 'Enable terminal', kOptionEnableTerminal,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(
                context, 'Enable TCP tunneling', kOptionEnableTunnel,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(
                context, 'Enable remote restart', kOptionEnableRemoteRestart,
                enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(
                context, 'Enable recording session', kOptionEnableRecordSession,
                enabled: enabled, fakeValue: fakeValue),
            if (isWindows)
              _OptionCheckBox(context, 'Enable blocking user input',
                  kOptionEnableBlockInput,
                  enabled: enabled, fakeValue: fakeValue),
            if (bind.mainSupportedPrivacyModeImpls() != '[]')
              _OptionCheckBox(
                  context, 'Enable privacy mode', kOptionEnablePrivacyMode,
                  enabled: enabled, fakeValue: fakeValue),
            _OptionCheckBox(context, 'Enable remote configuration modification',
                kOptionAllowRemoteConfigModification,
                enabled: enabled, fakeValue: fakeValue),
          ],
        ),
      ]);
    }

    return tmpWrapper();
  }
}

extension _SafetyMore on _SafetyState {
  Widget more(BuildContext context) {
    bool enabled = !locked;
    if (isWindows && !bind.isIncomingOnly()) {
      final zh = Localizations.localeOf(context).languageCode == 'zh';
      return Column(children: [
        _Card(title: 'Security', children: [
          _OptionCheckBox(context, 'Deny LAN discovery', 'enable-lan-discovery',
              reverse: true, enabled: enabled),
          ...directIp(context),
          whitelist(),
          idWhitelist(),
          ...autoDisconnect(context),
          _OptionCheckBox(context, 'keep-awake-during-incoming-sessions-label',
              kOptionKeepAwakeDuringIncomingSessions,
              reverse: false, enabled: enabled),
          if (bind.mainIsInstalled())
            _OptionCheckBox(context, 'allow-only-conn-window-open-tip',
                'allow-only-conn-window-open',
                reverse: false, enabled: enabled),
          if (bind.mainIsInstalled() && !isUnlockPinDisabled()) unlockPin()
        ]),
        _group(
            zh ? '高级' : 'Advanced',
            [
              shareRdp(context, enabled),
              PinnedSessionSetting(
                  enabled: enabled, leftMargin: _kContentHMargin),
            ],
            collapsible: true),
      ]);
    }
    return _Card(title: 'Security', children: [
      shareRdp(context, enabled),
      PinnedSessionSetting(enabled: enabled, leftMargin: _kContentHMargin),
      _OptionCheckBox(context, 'Deny LAN discovery', 'enable-lan-discovery',
          reverse: true, enabled: enabled),
      ...directIp(context),
      whitelist(),
      idWhitelist(),
      ...autoDisconnect(context),
      _OptionCheckBox(context, 'keep-awake-during-incoming-sessions-label',
          kOptionKeepAwakeDuringIncomingSessions,
          reverse: false, enabled: enabled),
      if (bind.mainIsInstalled())
        _OptionCheckBox(context, 'allow-only-conn-window-open-tip',
            'allow-only-conn-window-open',
            reverse: false, enabled: enabled),
      if (bind.mainIsInstalled() && !isUnlockPinDisabled()) unlockPin()
    ]);
  }

  shareRdp(BuildContext context, bool enabled) {
    onChanged(bool b) async {
      await bind.mainSetShareRdp(enable: b);
      _setState(() {});
    }

    bool value = bind.mainIsShareRdp();
    return Offstage(
      offstage: !(isWindows && bind.mainIsInstalled()),
      child: GestureDetector(
          child: Row(
            children: [
              Checkbox(
                      value: value,
                      onChanged: enabled ? (_) => onChanged(!value) : null)
                  .marginOnly(right: 5),
              Expanded(
                child: Text(translate('Enable RDP session sharing'),
                    style:
                        TextStyle(color: disabledTextColor(context, enabled))),
              )
            ],
          ).marginOnly(left: _kCheckBoxLeftMargin),
          onTap: enabled ? () => onChanged(!value) : null),
    );
  }
}
