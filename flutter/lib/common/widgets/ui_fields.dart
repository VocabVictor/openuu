import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/settings_row.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';

/// Controls shared by the dialogs built on [UiDialog]: a label above a
/// 32-high field with the error line under it, a switch row without on/off
/// text, and the body paragraph style.

OutlineInputBorder _border(Color color) => OutlineInputBorder(
    borderRadius: BorderRadius.circular(UiSpace.inputRadius),
    borderSide: BorderSide(color: color));

/// Body text of a dialog: 13/400 in the secondary colour.
Widget uiDialogText(String text) => Align(
    alignment: Alignment.centerLeft,
    child: Text(text,
        style: UiType.sidebarItem.copyWith(color: UiColor.textSecondary)));

/// A labelled field. [error] non-empty paints the border and the line under
/// the field in the danger colour; the line is always laid out so the dialog
/// does not jump when an error appears.
Widget uiDialogField(
  String label,
  TextEditingController controller, {
  String? error,
  FocusNode? focusNode,
  bool autoFocus = false,
  bool enabled = true,
  bool obscure = false,
  VoidCallback? onToggleObscure,
  VoidCallback? onSubmitted,
}) {
  final hasError = error != null && error.isNotEmpty;
  return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
    Text(label, style: UiType.caption),
    const SizedBox(height: UiSpace.fieldLabelGap),
    SizedBox(
        height: UiSpace.controlHeight,
        child: TextField(
            controller: controller,
            focusNode: focusNode,
            autofocus: autoFocus,
            enabled: enabled,
            obscureText: obscure,
            onSubmitted: onSubmitted == null ? null : (_) => onSubmitted(),
            style: UiType.sidebarItem.copyWith(color: UiColor.text),
            decoration: InputDecoration(
                filled: true,
                fillColor: Colors.white,
                isDense: true,
                contentPadding: const EdgeInsets.symmetric(
                    horizontal: UiSpace.inputPaddingX, vertical: 8),
                border: _border(UiColor.inputBorder),
                enabledBorder:
                    _border(hasError ? UiColor.danger : UiColor.inputBorder),
                focusedBorder:
                    _border(hasError ? UiColor.danger : UiColor.primary),
                suffixIcon: onToggleObscure == null
                    ? null
                    : IconButton(
                        iconSize: 16,
                        padding: EdgeInsets.zero,
                        constraints:
                            const BoxConstraints(minWidth: 28, minHeight: 28),
                        icon: Icon(
                            obscure
                                ? Icons.visibility_off_outlined
                                : Icons.visibility_outlined,
                            color: UiColor.muted),
                        onPressed: onToggleObscure)))),
    SizedBox(
        height: UiSpace.panelErrorHeight,
        child: hasError
            ? Text(error,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: UiType.caption.copyWith(color: UiColor.danger))
            : null),
  ]);
}

/// A switch row: label on the left, switch on the right, no on/off text.
Widget uiDialogToggle(String label, bool value, ValueChanged<bool>? onChanged) =>
    Row(children: [
      Expanded(
          child: Text(label,
              style: UiType.rowTitle.copyWith(fontWeight: FontWeight.w400))),
      SettingsSwitch(value: value, onChanged: onChanged),
    ]);
