import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/settings_row.dart';
import 'package:flutter_hbb/desktop/widgets/ui_palette.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';

/// Controls shared by the dialogs built on [UiDialog]: a label above a
/// 32-high field with the error line under it, a switch row without on/off
/// text, and the body paragraph style.

OutlineInputBorder _border(Color color) => OutlineInputBorder(
    borderRadius: BorderRadius.circular(UiSpace.inputRadius),
    borderSide: BorderSide(color: color));

/// Body text of a dialog: 13/400 in the secondary colour.
Widget uiDialogText(UiPalette pal, UiTypeset type, String text) => Align(
    alignment: Alignment.centerLeft,
    child: Text(text,
        style: type.sidebarItem.copyWith(color: pal.textSecondary)));

/// A labelled field. [error] non-empty paints the border and the line under
/// the field in the danger colour; the line is always laid out so the dialog
/// does not jump when an error appears.
Widget uiDialogField(
  UiPalette pal,
  UiTypeset type,
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
    Text(label, style: type.caption),
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
            style: type.sidebarItem.copyWith(color: pal.text),
            decoration: InputDecoration(
                filled: true,
                fillColor: pal.surface,
                isDense: true,
                contentPadding: const EdgeInsets.symmetric(
                    horizontal: UiSpace.inputPaddingX, vertical: 8),
                border: _border(pal.inputBorder),
                enabledBorder:
                    _border(hasError ? pal.danger : pal.inputBorder),
                focusedBorder:
                    _border(hasError ? pal.danger : pal.primary),
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
                            color: pal.muted),
                        onPressed: onToggleObscure)))),
    SizedBox(
        height: UiSpace.panelErrorHeight,
        child: hasError
            ? Text(error,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: type.caption.copyWith(color: pal.danger))
            : null),
  ]);
}

/// A switch row: label on the left, switch on the right, no on/off text.
Widget uiDialogToggle(UiTypeset type, String label, bool value,
        ValueChanged<bool>? onChanged) =>
    Row(children: [
      Expanded(
          child: Text(label,
              style: type.rowTitle.copyWith(fontWeight: FontWeight.w400))),
      SettingsSwitch(value: value, onChanged: onChanged),
    ]);
