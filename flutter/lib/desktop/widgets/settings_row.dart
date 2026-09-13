import 'package:flutter/material.dart';
import 'ui_tokens.dart';

/// One settings line: label (plus optional subtitle) on the left, a control
/// slot on the right. 48 high without subtitle, 56 with. The left padding is
/// fixed at 16; child rows indent through [SettingsChildRow], never here.
class SettingsRow extends StatelessWidget {
  final String label;
  final String? subtitle;
  final Widget control;
  final bool enabled;
  final VoidCallback? onTap;
  const SettingsRow(
      {super.key,
      required this.label,
      required this.control,
      this.subtitle,
      this.enabled = true,
      this.onTap});

  @override
  Widget build(BuildContext context) {
    final sub = subtitle ?? '';
    final row = Container(
        constraints: BoxConstraints(
            minHeight: sub.isEmpty
                ? UiSpace.settingsRowHeight
                : UiSpace.settingsRowHeightSub),
        padding:
            const EdgeInsets.symmetric(horizontal: UiSpace.settingsRowPaddingX),
        child: Row(children: [
          Expanded(
              child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(label, style: UiType.rowTitle),
                if (sub.isNotEmpty)
                  Text(sub,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: UiType.caption),
              ])),
          const SizedBox(width: UiSpace.settingsControlGap),
          control,
        ]));
    final body = enabled
        ? row
        : Opacity(opacity: UiSpace.settingsDisabledOpacity, child: row);
    if (onTap == null) return body;
    return InkWell(
        onTap: enabled ? onTap : null,
        hoverColor: UiColor.settingsRowHover,
        child: body);
  }
}

/// A row nested under its parent: indented 20 with a 2px rail on the left.
class SettingsChildRow extends StatelessWidget {
  final Widget child;
  const SettingsChildRow({super.key, required this.child});

  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(left: UiSpace.settingsChildIndent),
      child: Container(
          decoration: const BoxDecoration(
              border: Border(
                  left: BorderSide(
                      color: UiColor.border, width: UiSpace.settingsChildRail))),
          child: child));
}

/// A titled group: the title sits above a bordered card that stacks its rows
/// with inset dividers between them. A collapsible group (an "Advanced"
/// section) shows a chevron before its title and starts collapsed.
class SettingsGroup extends StatefulWidget {
  final String? title;
  final Widget? titleTrailing;
  final List<Widget> children;
  final bool collapsible;
  const SettingsGroup(
      {super.key,
      this.title,
      this.titleTrailing,
      required this.children,
      this.collapsible = false});

  @override
  State<SettingsGroup> createState() => _SettingsGroupState();
}

class _SettingsGroupState extends State<SettingsGroup> {
  late bool _open = !widget.collapsible;

  @override
  Widget build(BuildContext context) {
    final title = widget.title;
    final titleRow = Row(children: [
      if (widget.collapsible) ...[
        AnimatedRotation(
            turns: _open ? 0 : -.25,
            duration: UiSpace.panelDuration,
            child: const Icon(Icons.expand_more,
                size: UiSpace.groupChevronSize, color: UiColor.muted)),
        const SizedBox(width: UiSpace.groupChevronGap),
      ],
      Expanded(child: Text(title ?? '', style: UiType.groupTitle)),
      if (widget.titleTrailing != null) widget.titleTrailing!,
    ]);
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      if (title != null) ...[
        SizedBox(
            height: UiSpace.settingsGroupTitleHeight,
            child: widget.collapsible
                ? InkWell(
                    onTap: () => setState(() => _open = !_open),
                    child: titleRow)
                : titleRow),
        const SizedBox(height: UiSpace.settingsGroupTitleGap),
      ],
      if (_open)
        Container(
            decoration: BoxDecoration(
                color: Colors.white,
                borderRadius:
                    BorderRadius.circular(UiSpace.settingsGroupRadius),
                border: Border.all(color: UiColor.border)),
            clipBehavior: Clip.antiAlias,
            child: Column(children: [
              for (var i = 0; i < widget.children.length; i++) ...[
                if (i > 0)
                  const Divider(
                      height: 1,
                      indent: UiSpace.settingsRowPaddingX,
                      endIndent: UiSpace.settingsRowPaddingX,
                      color: UiColor.settingsDivider),
                widget.children[i],
              ],
            ])),
    ]);
  }
}

/// The settings switch: a 36×20 track, no label text, 36×28 hit area.
class SettingsSwitch extends StatelessWidget {
  final bool value;
  final ValueChanged<bool>? onChanged;
  const SettingsSwitch({super.key, required this.value, this.onChanged});

  @override
  Widget build(BuildContext context) => SizedBox(
      width: UiSpace.settingsSwitchWidth,
      height: UiSpace.settingsSwitchHitHeight,
      child: FittedBox(
          fit: BoxFit.contain,
          child: Switch(
              value: value,
              activeColor: Colors.white,
              activeTrackColor: UiColor.primary,
              inactiveThumbColor: Colors.white,
              inactiveTrackColor: UiColor.settingsSwitchOff,
              trackOutlineColor: WidgetStateProperty.all(Colors.transparent),
              materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
              onChanged: onChanged)));
}
