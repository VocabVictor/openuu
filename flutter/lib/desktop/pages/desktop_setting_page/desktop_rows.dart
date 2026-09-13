part of 'desktop_setting_page.dart';

// Desktop-shell rows: every control sits in a SettingsRow, the row label
// starts at the fixed left padding and the control is on the right.

Widget _switchRow(BuildContext context, String label, bool value,
    ValueChanged<bool>? onChanged,
    {bool enabled = true, String? description}) =>
    _settingRow(context, label,
        SettingsSwitch(value: value, onChanged: enabled ? onChanged : null),
        enabled: enabled, description: description);

/// The secondary button of the settings page: white, 1px border, 28 high.
Widget _secondaryButton(BuildContext context, String label, VoidCallback? onPressed,
        {double height = UiSpace.settingsControlHeight}) =>
    SizedBox(
    height: height,
    child: OutlinedButton(
        style: OutlinedButton.styleFrom(
            foregroundColor: UiColor.of(context).text,
            backgroundColor: Colors.white,
            side: BorderSide(color: UiColor.of(context).inputBorder),
            padding: const EdgeInsets.symmetric(horizontal: UiSpace.s3),
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(UiSpace.buttonRadius)),
            textStyle: UiType.of(context).button),
        onPressed: onPressed,
        child: Text(translate(label))));

/// The primary button, 28 high, for a panel's Save.
Widget _primaryButton(BuildContext context, String label, VoidCallback? onPressed,
        {double height = UiSpace.settingsControlHeight}) =>
    SizedBox(
    height: height,
    child: ElevatedButton(
        style: ElevatedButton.styleFrom(
            backgroundColor: UiColor.of(context).primary,
            foregroundColor: Colors.white,
            disabledBackgroundColor: UiColor.of(context).primaryDisabled,
            disabledForegroundColor: Colors.white,
            elevation: 0,
            padding: const EdgeInsets.symmetric(horizontal: UiSpace.s4),
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(UiSpace.buttonRadius)),
            textStyle: UiType.of(context).button),
        onPressed: onPressed,
        child: Text(translate(label))));

/// The danger button: white, red border and text; confirmation lives in
/// the dialog that opens it.
Widget _dangerButton(BuildContext context, String label, VoidCallback? onPressed,
        {double height = UiSpace.settingsControlHeight}) =>
    SizedBox(
        height: height,
        child: OutlinedButton(
            style: OutlinedButton.styleFrom(
                foregroundColor: UiColor.of(context).danger,
                backgroundColor: Colors.white,
                side: BorderSide(color: UiColor.of(context).dangerBorder),
                padding: const EdgeInsets.symmetric(horizontal: UiSpace.s3),
                shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(UiSpace.buttonRadius)),
                textStyle: UiType.of(context).button),
            onPressed: onPressed,
            child: Text(translate(label))));

/// A secret shown as "set" / "not set" with a Set / Change button; the value
/// itself never appears on the page.
Widget _secretRow(BuildContext context, String label, bool isSet,
    VoidCallback? onEdit,
    {bool enabled = true, String? description}) {
  final zh = Localizations.localeOf(context).languageCode == 'zh';
  return _settingRow(
      context,
      label,
      Row(mainAxisSize: MainAxisSize.min, children: [
        Text(isSet ? (zh ? '已设置' : 'Set') : (zh ? '未设置' : 'Not set'),
            style: UiType.of(context).caption
                .copyWith(color: isSet ? UiColor.of(context).textSecondary : UiColor.of(context).faint)),
        const SizedBox(width: UiSpace.settingsControlGap),
        _secondaryButton(context, 
            isSet ? (zh ? '修改' : 'Change') : (zh ? '设置' : 'Set'),
            enabled ? onEdit : null),
      ]),
      enabled: enabled,
      description: description);
}

/// A child row: indented under its parent with the rail on the left.
Widget _childRow(Widget row) => SettingsChildRow(child: row);

/// The Apply button next to a numeric field: secondary on the desktop shell.
Widget _applyButton(BuildContext context, VoidCallback? onPressed) => isWindows &&
        !bind.isIncomingOnly()
    ? _secondaryButton(context, 'Apply', onPressed)
    : ElevatedButton(onPressed: onPressed, child: Text(translate('Apply')));

/// A numeric field, 56 wide and 28 high on the desktop shell.
Widget _numberField(BuildContext context, TextEditingController controller,
        {required bool enabled,
        required String hint,
        required ValueChanged<String> onChanged,
        required List<TextInputFormatter> inputFormatters}) =>
    isWindows && !bind.isIncomingOnly()
        ? SizedBox(
            width: UiSpace.settingsNumberFieldWidth,
            height: UiSpace.settingsControlHeight,
            child: TextField(
                controller: controller,
                enabled: enabled,
                onChanged: onChanged,
                inputFormatters: inputFormatters,
                textAlign: TextAlign.right,
                style: UiType.of(context).rowTitle.copyWith(
                    fontSize: 13, fontWeight: FontWeight.w400),
                decoration: InputDecoration(
                    hintText: hint,
                    hintStyle: UiType.of(context).caption.copyWith(color: UiColor.of(context).faint),
                    isDense: true,
                    contentPadding: const EdgeInsets.symmetric(
                        horizontal: UiSpace.s2, vertical: 6),
                    border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(UiSpace.inputRadius),
                        borderSide: BorderSide(color: UiColor.of(context).inputBorder)),
                    enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(UiSpace.inputRadius),
                        borderSide:
                            BorderSide(color: UiColor.of(context).inputBorder)))))
        : SizedBox(
            width: 95,
            child: TextField(
              controller: controller,
              enabled: enabled,
              onChanged: onChanged,
              inputFormatters: inputFormatters,
              decoration: InputDecoration(
                hintText: hint,
                contentPadding:
                    const EdgeInsets.symmetric(vertical: 12, horizontal: 12),
              ),
            ).workaroundFreezeLinuxMint().marginOnly(right: 15),
          );
