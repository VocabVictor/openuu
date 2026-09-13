import 'package:flutter/material.dart';
import 'ui_tokens.dart';

/// The settings dropdown: a fixed-width, 28-high trigger (160 for short
/// values, 200 for medium, 240 for device names and codecs) whose menu is
/// anchored under it, at least as wide, with the current value ticked.
class SettingsDropdown extends StatelessWidget {
  final List<String> keys;
  final List<String> values;
  final String current;
  final ValueChanged<String> onChanged;
  final double width;
  final bool enabled;
  const SettingsDropdown(
      {super.key,
      required this.keys,
      required this.values,
      required this.current,
      required this.onChanged,
      this.width = 160,
      this.enabled = true});

  @override
  Widget build(BuildContext context) {
    var index = keys.indexOf(current);
    if (index < 0) index = 0;
    final label = values.isEmpty ? '' : values[index];
    final textStyle = UiType.of(context).rowTitle.copyWith(
        fontSize: 13,
        fontWeight: FontWeight.w400,
        color: enabled ? UiColor.of(context).text : UiColor.of(context).faint);
    return SizedBox(
        width: width,
        height: UiSpace.settingsControlHeight,
        child: PopupMenuButton<String>(
            enabled: enabled,
            tooltip: '',
            position: PopupMenuPosition.under,
            offset: const Offset(0, UiSpace.menuOffset),
            constraints: BoxConstraints(minWidth: width),
            padding: EdgeInsets.zero,
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(UiSpace.menuRadius),
                side: BorderSide(color: UiColor.of(context).border)),
            elevation: 4,
            color: UiColor.of(context).surface,
            onSelected: onChanged,
            itemBuilder: (_) => [
                  for (var i = 0; i < keys.length; i++)
                    PopupMenuItem<String>(
                        value: keys[i],
                        height: UiSpace.settingsControlHeight,
                        padding: const EdgeInsets.symmetric(
                            horizontal: UiSpace.menuItemPaddingX),
                        child: Row(children: [
                          Expanded(
                              child: Text(values[i],
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: UiType.of(context).rowTitle.copyWith(
                                      fontWeight: FontWeight.w400))),
                          if (i == index)
                            Icon(Icons.check,
                                size: 14, color: UiColor.of(context).primary),
                        ])),
                ],
            child: Container(
                padding: const EdgeInsets.only(
                    left: UiSpace.settingsDropdownPaddingLeft,
                    right: UiSpace.settingsDropdownPaddingRight),
                decoration: BoxDecoration(
                    color: UiColor.of(context).surface,
                    borderRadius: BorderRadius.circular(UiSpace.inputRadius),
                    border: Border.all(color: UiColor.of(context).inputBorder)),
                child: Row(children: [
                  Expanded(
                      child: Text(label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: textStyle)),
                  Icon(Icons.expand_more, size: 14, color: UiColor.of(context).muted),
                ]))));
  }
}
