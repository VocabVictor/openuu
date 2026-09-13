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
Widget _secondaryButton(String label, VoidCallback? onPressed) => SizedBox(
    height: UiSpace.settingsControlHeight,
    child: OutlinedButton(
        style: OutlinedButton.styleFrom(
            foregroundColor: UiColor.text,
            backgroundColor: Colors.white,
            side: const BorderSide(color: UiColor.inputBorder),
            padding: const EdgeInsets.symmetric(horizontal: UiSpace.s3),
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(UiSpace.buttonRadius)),
            textStyle: UiType.button),
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
            style: UiType.caption
                .copyWith(color: isSet ? UiColor.textSecondary : UiColor.faint)),
        const SizedBox(width: UiSpace.settingsControlGap),
        _secondaryButton(
            isSet ? (zh ? '修改' : 'Change') : (zh ? '设置' : 'Set'),
            enabled ? onEdit : null),
      ]),
      enabled: enabled,
      description: description);
}

/// A child row: indented under its parent with the rail on the left.
Widget _childRow(Widget row) => SettingsChildRow(child: row);
