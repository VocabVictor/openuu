import 'package:flutter/material.dart';
import 'settings_row.dart';
import 'ui_tokens.dart';

/// A settings row that expands into an in-shell panel (design-review-settings
/// §2.6): the row keeps its value summary, the chevron turns, and the panel
/// opens below it on a slightly darker ground with the child indent.
class SettingsExpandPanel extends StatefulWidget {
  final String label;
  final String summary;
  final bool enabled;
  /// Builds the panel; `close` collapses it (after a save, for example).
  final Widget Function(BuildContext context, VoidCallback close) panel;
  const SettingsExpandPanel(
      {super.key,
      required this.label,
      required this.summary,
      required this.panel,
      this.enabled = true});

  @override
  State<SettingsExpandPanel> createState() => _SettingsExpandPanelState();
}

class _SettingsExpandPanelState extends State<SettingsExpandPanel> {
  bool _open = false;

  void _toggle() => setState(() => _open = !_open);

  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        SettingsRow(
            label: widget.label,
            enabled: widget.enabled,
            onTap: _toggle,
            control: Row(mainAxisSize: MainAxisSize.min, children: [
              AnimatedOpacity(
                  opacity: _open ? 0 : 1,
                  duration: UiSpace.panelFadeDuration,
                  child: Text(widget.summary, style: UiType.of(context).caption)),
              const SizedBox(width: UiSpace.s2),
              AnimatedRotation(
                  turns: _open ? .5 : 0,
                  duration: UiSpace.panelDuration,
                  child: Icon(Icons.expand_more,
                      size: 16, color: UiColor.of(context).muted)),
            ])),
        AnimatedSize(
            duration: UiSpace.panelDuration,
            curve: Curves.easeOut,
            alignment: Alignment.topCenter,
            child: _open
                ? Container(
                    color: UiColor.of(context).panelBg,
                    padding: const EdgeInsets.fromLTRB(
                        UiSpace.panelPaddingLeft,
                        UiSpace.panelPaddingY,
                        UiSpace.panelPaddingRight,
                        UiSpace.panelPaddingY),
                    child: widget.panel(context, () {
                      if (mounted) setState(() => _open = false);
                    }))
                : const SizedBox(width: double.infinity)),
      ]);
}

/// A labelled field inside a panel: 12px label above, a 320-wide 32-high
/// input, and a 16-high error slot so the layout never jumps.
class SettingsPanelField extends StatelessWidget {
  final String label;
  final TextEditingController controller;
  final String error;
  final String? hint;
  final bool obscure;
  final bool monospace;
  final int lines;
  final bool enabled;
  const SettingsPanelField(
      {super.key,
      required this.label,
      required this.controller,
      this.error = '',
      this.hint,
      this.obscure = false,
      this.monospace = false,
      this.lines = 1,
      this.enabled = true});

  @override
  Widget build(BuildContext context) {
    OutlineInputBorder border(Color color) => OutlineInputBorder(
        borderRadius: BorderRadius.circular(UiSpace.inputRadius),
        borderSide: BorderSide(color: color));
    final style = UiType.of(context).rowTitle.copyWith(
        fontSize: monospace ? 12 : 13,
        fontWeight: FontWeight.w400,
        fontFamily: monospace ? 'Consolas' : null,
        fontFamilyFallback: monospace ? const ['Courier New'] : null);
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text(label, style: UiType.of(context).caption),
      const SizedBox(height: UiSpace.fieldLabelGap),
      SizedBox(
          width: UiSpace.panelFieldWidth,
          height: lines > 1 ? UiSpace.panelTextAreaHeight : UiSpace.controlHeight,
          child: TextField(
              controller: controller,
              enabled: enabled,
              obscureText: obscure,
              maxLines: lines,
              expands: lines > 1,
              textAlignVertical: TextAlignVertical.top,
              style: style,
              decoration: InputDecoration(
                  hintText: hint,
                  hintStyle: UiType.of(context).caption.copyWith(color: UiColor.of(context).faint),
                  filled: true,
                  fillColor: UiColor.of(context).surface,
                  isDense: true,
                  contentPadding: const EdgeInsets.symmetric(
                      horizontal: UiSpace.inputPaddingX, vertical: 8),
                  border: border(UiColor.of(context).inputBorder),
                  enabledBorder:
                      border(error.isEmpty ? UiColor.of(context).inputBorder : UiColor.of(context).danger),
                  focusedBorder:
                      border(error.isEmpty ? UiColor.of(context).primary : UiColor.of(context).danger)))),
      SizedBox(
          height: UiSpace.panelErrorHeight,
          child: error.isEmpty
              ? null
              : Text(error,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: UiType.of(context).caption.copyWith(color: UiColor.of(context).danger))),
    ]);
  }
}

/// The panel's button bar: right-aligned, 8 apart, 16 below the last field.
class SettingsPanelFooter extends StatelessWidget {
  final List<Widget> buttons;
  const SettingsPanelFooter({super.key, required this.buttons});

  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(top: UiSpace.panelFooterGap),
      child: Row(mainAxisAlignment: MainAxisAlignment.end, children: [
        for (var i = 0; i < buttons.length; i++) ...[
          if (i > 0) const SizedBox(width: UiSpace.s2),
          buttons[i],
        ],
      ]));
}
