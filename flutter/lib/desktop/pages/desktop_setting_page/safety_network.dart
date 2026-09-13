part of 'desktop_setting_page.dart';

extension _SafetyNetwork on _SafetyState {
  List<Widget> directIp(BuildContext context) {
    TextEditingController controller = TextEditingController();
    update(bool v) => _setState(() {});
    RxBool applyEnabled = false.obs;
    return [
      _OptionCheckBox(context, 'Enable direct IP access', kOptionDirectServer,
          update: update, enabled: !locked),
      () {
        // Simple temp wrapper for PR check
        tmpWrapper() {
          bool enabled = option2bool(kOptionDirectServer,
              bind.mainGetOptionSync(key: kOptionDirectServer));
          if (!enabled) applyEnabled.value = false;
          controller.text =
              bind.mainGetOptionSync(key: kOptionDirectAccessPort);
          final isOptFixed = isOptionFixed(kOptionDirectAccessPort);
          return Offstage(
            offstage: !enabled,
            child: _SubLabeledWidget(
              context,
              'Port',
              Row(mainAxisSize: MainAxisSize.min, children: [
                _numberField(context, controller,
                    enabled: enabled && !locked && !isOptFixed,
                    hint: '21118',
                    onChanged: (_) => applyEnabled.value = true,
                    inputFormatters: [
                      FilteringTextInputFormatter.allow(RegExp(
                          r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                    ]),
                const SizedBox(width: UiSpace.s2),
                Obx(() => _applyButton(applyEnabled.value &&
                        enabled &&
                        !locked &&
                        !isOptFixed
                    ? () async {
                        applyEnabled.value = false;
                        await bind.mainSetOption(
                            key: kOptionDirectAccessPort,
                            value: controller.text);
                      }
                    : null))
              ]),
              enabled: enabled && !locked && !isOptFixed,
            ),
          );
        }

        return tmpWrapper();
      }(),
    ];
  }

  Widget whitelist() {
    bool enabled = !locked;
    // Simple temp wrapper for PR check
    tmpWrapper() {
      RxBool hasWhitelist = whitelistNotEmpty().obs;
      update() async {
        hasWhitelist.value = whitelistNotEmpty();
      }

      onChanged(bool? checked) async {
        changeWhiteList(callback: update);
      }

      final isOptFixed = isOptionFixed(kOptionWhitelist);
      if (isWindows && !bind.isIncomingOnly()) {
        return Obx(() => _switchRow(context, 'Use IP Whitelisting',
            hasWhitelist.value, (_) => onChanged(!hasWhitelist.value),
            enabled: enabled && !isOptFixed,
            description: translate('whitelist_tip')));
      }
      return GestureDetector(
        child: Tooltip(
          message: translate('whitelist_tip'),
          child: Obx(() => Row(
                children: [
                  Checkbox(
                          value: hasWhitelist.value,
                          onChanged: enabled && !isOptFixed ? onChanged : null)
                      .marginOnly(right: 5),
                  Offstage(
                    offstage: !hasWhitelist.value,
                    child: MouseRegion(
                      child: const Icon(Icons.warning_amber_rounded,
                              color: Color.fromARGB(255, 255, 204, 0))
                          .marginOnly(right: 5),
                      cursor: SystemMouseCursors.click,
                    ),
                  ),
                  Expanded(
                      child: Text(
                    translate('Use IP Whitelisting'),
                    style:
                        TextStyle(color: disabledTextColor(context, enabled)),
                  ))
                ],
              )),
        ),
        onTap: enabled
            ? () {
                onChanged(!hasWhitelist.value);
              }
            : null,
      ).marginOnly(left: _kCheckBoxLeftMargin);
    }

    return tmpWrapper();
  }

  Widget idWhitelist() {
    bool enabled = !locked;
    RxBool hasIdWhitelist = idWhitelistNotEmpty().obs;
    update() async {
      hasIdWhitelist.value = idWhitelistNotEmpty();
    }

    onChanged(bool? checked) async {
      changeIdWhiteList(callback: update);
    }

    final isOptFixed = isOptionFixed(kOptionIdWhitelist);
    if (isWindows && !bind.isIncomingOnly()) {
      return Obx(() => _switchRow(context, 'Use ID whitelisting',
          hasIdWhitelist.value, (_) => onChanged(!hasIdWhitelist.value),
          enabled: enabled && !isOptFixed,
          description: translate('id_whitelist_tip')));
    }
    return GestureDetector(
      child: Tooltip(
        message: translate('id_whitelist_tip'),
        child: Obx(() => Row(
              children: [
                Checkbox(
                        value: hasIdWhitelist.value,
                        onChanged: enabled && !isOptFixed ? onChanged : null)
                    .marginOnly(right: 5),
                Offstage(
                  offstage: !hasIdWhitelist.value,
                  child: MouseRegion(
                    child: const Icon(Icons.warning_amber_rounded,
                            color: Color.fromARGB(255, 255, 204, 0))
                        .marginOnly(right: 5),
                    cursor: SystemMouseCursors.click,
                  ),
                ),
                Expanded(
                    child: Text(
                  translate('Use ID whitelisting'),
                  style: TextStyle(color: disabledTextColor(context, enabled)),
                ))
              ],
            )),
      ),
      onTap: enabled
          ? () {
              onChanged(!hasIdWhitelist.value);
            }
          : null,
    ).marginOnly(left: _kCheckBoxLeftMargin);
  }
}
