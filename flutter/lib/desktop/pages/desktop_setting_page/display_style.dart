part of 'desktop_setting_page.dart';

extension _DisplayStyle on _DisplayState {
  Widget _choiceCard(BuildContext context, String title, String current,
      Map<String, String> options, void Function(String) onChanged,
      {bool enabled = true, Widget? detail, double width = 200}) {
    // The rows sit in the "defaults for new connections" group, so the
    // "Default" prefix of the legacy keys is dropped from the label (V7).
    final label = translate(title)
        .replaceFirst(RegExp(r'^(Default |默认)'), '');
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      _settingRow(
          context,
          title,
          labelText: label,
          SettingsDropdown(
            keys: options.keys.toList(),
            values: options.values.map(translate).toList(),
            current: current,
            width: width,
            enabled: enabled,
            onChanged: onChanged,
          ),
          enabled: enabled),
      if (detail != null)
        Padding(
            padding: const EdgeInsets.fromLTRB(
                UiSpace.settingsRowPaddingX, UiSpace.s2, UiSpace.settingsRowPaddingX, UiSpace.s4),
            child: detail),
    ]);
  }

  Widget viewStyle(BuildContext context) {
    final isOptFixed = isOptionFixed(kOptionViewStyle);
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(key: kOptionViewStyle, value: value);
      _setState(() {});
    }

    final groupValue = bind.mainGetUserDefaultOption(key: kOptionViewStyle);
    if (isWindows && !bind.isIncomingOnly()) {
      return _choiceCard(context, 'Default View Style', groupValue, {
        kRemoteViewStyleOriginal: 'Scale original',
        kRemoteViewStyleAdaptive: 'Scale adaptive',
      }, onChanged, enabled: !isOptFixed);
    }
    return _Card(title: 'Default View Style', children: [
      _Radio(context,
          value: kRemoteViewStyleOriginal,
          groupValue: groupValue,
          label: 'Scale original',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: kRemoteViewStyleAdaptive,
          groupValue: groupValue,
          label: 'Scale adaptive',
          onChanged: isOptFixed ? null : onChanged),
    ]);
  }

  Widget scrollStyle(BuildContext context) {
    final isOptFixed = isOptionFixed(kOptionScrollStyle);
    onChanged(String value) async {
      await bind.mainSetUserDefaultOption(
          key: kOptionScrollStyle, value: value);
      _setState(() {});
    }

    final groupValue = bind.mainGetUserDefaultOption(key: kOptionScrollStyle);

    onEdgeScrollEdgeThicknessChanged(double value) async {
      await bind.mainSetUserDefaultOption(
          key: kOptionEdgeScrollEdgeThickness, value: value.round().toString());
      _setState(() {});
    }

    if (isWindows && !bind.isIncomingOnly()) {
      return _choiceCard(context, 'Default Scroll Style', groupValue, {
        kRemoteScrollStyleAuto: 'ScrollAuto',
        kRemoteScrollStyleBar: 'Scrollbar',
        kRemoteScrollStyleEdge: 'ScrollEdge',
      }, onChanged, enabled: !isOptFixed,
        detail: groupValue == kRemoteScrollStyleEdge ? EdgeThicknessControl(
          value: double.tryParse(bind.mainGetUserDefaultOption(
            key: kOptionEdgeScrollEdgeThickness)) ?? 100.0,
          onChanged: isOptionFixed(kOptionEdgeScrollEdgeThickness)
            ? null : onEdgeScrollEdgeThicknessChanged,
        ) : null);
    }
    return _Card(title: 'Default Scroll Style', children: [
      _Radio(context,
          value: kRemoteScrollStyleAuto,
          groupValue: groupValue,
          label: 'ScrollAuto',
          onChanged: isOptFixed ? null : onChanged),
      _Radio(context,
          value: kRemoteScrollStyleBar,
          groupValue: groupValue,
          label: 'Scrollbar',
          onChanged: isOptFixed ? null : onChanged),
      ...[
        _Radio(context,
            value: kRemoteScrollStyleEdge,
            groupValue: groupValue,
            label: 'ScrollEdge',
            onChanged: isOptFixed ? null : onChanged),
        Offstage(
            offstage: groupValue != kRemoteScrollStyleEdge,
            child: EdgeThicknessControl(
              value: double.tryParse(bind.mainGetUserDefaultOption(
                      key: kOptionEdgeScrollEdgeThickness)) ??
                  100.0,
              onChanged: isOptionFixed(kOptionEdgeScrollEdgeThickness)
                  ? null
                  : onEdgeScrollEdgeThicknessChanged,
            )),
      ],
    ]);
  }
}
