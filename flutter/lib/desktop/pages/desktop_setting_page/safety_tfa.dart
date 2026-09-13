part of 'desktop_setting_page.dart';

extension _SafetyTfa on _SafetyState {
  Widget tfa() {
    bool enabled = !locked;
    // Simple temp wrapper for PR check
    tmpWrapper() {
      RxBool has2fa = bind.mainHasValid2FaSync().obs;
      RxBool hasBot = bind.mainHasValidBotSync().obs;
      update() async {
        has2fa.value = bind.mainHasValid2FaSync();
        _setState(() {});
      }

      onChanged(bool? checked) async {
        if (checked == false) {
          CommonConfirmDialog(
              gFFI.dialogManager, translate('cancel-2fa-confirm-tip'), () {
            change2fa(callback: update);
          });
        } else {
          change2fa(callback: update);
        }
      }

      final tfa = GestureDetector(
        child: InkWell(
          child: Obx(() => Row(
                children: [
                  Checkbox(
                          value: has2fa.value,
                          onChanged: enabled ? onChanged : null)
                      .marginOnly(right: 5),
                  Expanded(
                      child: Text(
                    translate('enable-2fa-title'),
                    style:
                        TextStyle(color: disabledTextColor(context, enabled)),
                  ))
                ],
              )),
        ),
        onTap: () {
          onChanged(!has2fa.value);
        },
      ).marginOnly(left: _kCheckBoxLeftMargin);
      final desktop = isWindows && !bind.isIncomingOnly();
      if (!has2fa.value && !desktop) {
        return tfa;
      }
      updateBot() async {
        hasBot.value = bind.mainHasValidBotSync();
        _setState(() {});
      }

      onChangedBot(bool? checked) async {
        if (checked == false) {
          CommonConfirmDialog(
              gFFI.dialogManager, translate('cancel-bot-confirm-tip'), () {
            changeBot(callback: updateBot);
          });
        } else {
          changeBot(callback: updateBot);
        }
      }

      final bot = GestureDetector(
        child: Tooltip(
          waitDuration: Duration(milliseconds: 300),
          message: translate("enable-bot-tip"),
          child: InkWell(
              child: Obx(() => Row(
                    children: [
                      Checkbox(
                              value: hasBot.value,
                              onChanged: enabled ? onChangedBot : null)
                          .marginOnly(right: 5),
                      Expanded(
                          child: Text(
                        translate('Telegram bot'),
                        style: TextStyle(
                            color: disabledTextColor(context, enabled)),
                      ))
                    ],
                  ))),
        ),
        onTap: () {
          onChangedBot(!hasBot.value);
        },
      ).marginOnly(left: _kCheckBoxLeftMargin + 30);

      final trust = Row(
        children: [
          Flexible(
            child: Tooltip(
              waitDuration: Duration(milliseconds: 300),
              message: translate("enable-trusted-devices-tip"),
              child: _OptionCheckBox(context, "Enable trusted devices",
                  kOptionEnableTrustedDevices,
                  enabled: !locked, update: (v) {
                _setState(() {});
              }),
            ),
          ),
          if (mainGetBoolOptionSync(kOptionEnableTrustedDevices))
            ElevatedButton(
                onPressed: locked
                    ? null
                    : () {
                        manageTrustedDeviceDialog();
                      },
                child: Text(translate('Manage trusted devices')))
        ],
      ).marginOnly(left: 30);

      if (desktop) {
        return Obx(() => Column(children: [
              _switchRow(context, 'enable-2fa-title', has2fa.value,
                  (_) => onChanged(!has2fa.value),
                  enabled: enabled, description: ''),
              if (has2fa.value)
                _childRow(_switchRow(context, 'Telegram bot', hasBot.value,
                    (_) => onChangedBot(!hasBot.value),
                    enabled: enabled,
                    description: translate('enable-bot-tip'))),
              if (has2fa.value)
                _childRow(_OptionCheckBox(
                    context, "Enable trusted devices", kOptionEnableTrustedDevices,
                    enabled: !locked,
                    description: translate('enable-trusted-devices-tip'),
                    update: (v) => _setState(() {}))),
              if (has2fa.value &&
                  mainGetBoolOptionSync(kOptionEnableTrustedDevices))
                _childRow(_settingRow(
                    context,
                    'Manage trusted devices',
                    _secondaryButton('Manage trusted devices',
                        locked ? null : manageTrustedDeviceDialog),
                    description: '')),
            ]));
      }
      return Column(
        children: [tfa, bot, trust],
      );
    }

    return tmpWrapper();
  }

  Widget changeId() {
    return ChangeNotifierProvider.value(
        value: gFFI.serverModel,
        child: Consumer<ServerModel>(builder: ((context, model, child) {
          final button = _Button('Change ID', changeIdDialog,
              enabled: !locked && model.connectStatus > 0);
          if (isWindows && !bind.isIncomingOnly()) {
            return _settingRow(context, 'Change ID', button, description: '');
          }
          return button;
        })));
  }
}
