import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';

/// One button of a [UiDialog]'s action row.
///
/// A dialog has at most one primary action and it is the last one; the
/// secondary and danger buttons come before it in the order given.
class UiDialogAction {
  const UiDialogAction._(this.label, this.onPressed, this.kind);

  const UiDialogAction.primary(String label, VoidCallback? onPressed)
      : this._(label, onPressed, UiDialogActionKind.primary);
  const UiDialogAction.secondary(String label, VoidCallback? onPressed)
      : this._(label, onPressed, UiDialogActionKind.secondary);
  const UiDialogAction.danger(String label, VoidCallback? onPressed)
      : this._(label, onPressed, UiDialogActionKind.danger);

  /// Passed through [translate].
  final String label;
  final VoidCallback? onPressed;
  final UiDialogActionKind kind;

  bool get isPrimary => kind == UiDialogActionKind.primary;
}

enum UiDialogActionKind { primary, secondary, danger }

/// The generic dialog shell (design-review-settings.md §2.6,
/// docs/session-window-restyle.md): 352 wide content with 24 padding, a
/// 15/600 left-aligned title with an optional close icon, the body, and a
/// right-aligned button row. No large icon.
///
/// Enter triggers the primary action and Escape calls [onClose]; a dialog
/// that must not be dismissable by Escape simply omits [onClose].
///
/// A dialog that asks the user to trust something keeps the permissive and
/// the safe choice equally reachable (docs/cm-restyle-plan.md): both are
/// full-size buttons with a border, neither is a bare text link, and the
/// safe one is the primary.
class UiDialog extends StatelessWidget {
  UiDialog({
    Key? key,
    required this.title,
    required this.body,
    this.actions = const [],
    this.onClose,
    this.width = UiSpace.dialogContentWidth,
  })  : assert(_atMostOnePrimaryLast(actions)),
        super(key: key);

  final String title;
  final Widget body;
  final List<UiDialogAction> actions;

  /// Shows the close icon in the title row and handles Escape.
  final VoidCallback? onClose;
  final double width;

  static bool _atMostOnePrimaryLast(List<UiDialogAction> actions) {
    final primaries = actions.where((a) => a.isPrimary).length;
    return primaries == 0 || (primaries == 1 && actions.last.isPrimary);
  }

  @override
  Widget build(BuildContext context) => alert(context);

  /// The dialog as a [CustomAlertDialog], for `dialogManager.show`, whose
  /// builder has to return that type.
  CustomAlertDialog alert(BuildContext context) {
    final primary = actions.isNotEmpty && actions.last.isPrimary
        ? actions.last.onPressed
        : null;
    // Escape runs [onClose] and nothing else. It used to fall back to the
    // first secondary action, which is unsafe for a dialog that asks the user
    // to trust something: there the first secondary can be the permissive
    // choice ("continue anyway"), and Escape must never take it. A dialog
    // that should be dismissable by Escape passes onClose.
    final cancel = onClose;
    return CustomAlertDialog(
      titlePadding: EdgeInsets.zero,
      // A fixed width only on the desktop shell. On a phone the dialog has
      // about 280dp to live in once the Material inset and the content
      // padding are taken off, so demanding 352 would overflow the screen;
      // there the width is an upper bound and the dialog shrinks to fit.
      contentBoxConstraints: BoxConstraints(
          minWidth: isDesktop ? width : 0, maxWidth: width),
      onSubmit: primary,
      onCancel: cancel,
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _titleRow(),
          const SizedBox(height: UiSpace.s2),
          body,
          if (actions.isNotEmpty) ...[
            const SizedBox(height: UiSpace.s6),
            Row(mainAxisAlignment: MainAxisAlignment.end, children: [
              for (var i = 0; i < actions.length; i++) ...[
                if (i > 0) const SizedBox(width: UiSpace.s2),
                uiDialogButton(actions[i]),
              ],
            ]),
          ],
        ],
      ),
    );
  }

  Widget _titleRow() => SizedBox(
      height: UiSpace.dialogTitleHeight,
      child: Row(children: [
        Expanded(child: Text(title, style: UiType.sectionTitle)),
        if (onClose != null)
          IconButton(
              iconSize: 16,
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
              icon: const Icon(Icons.close, color: UiColor.muted),
              onPressed: onClose),
      ]));
}

/// A 32-high dialog button in the primary / secondary / danger style.
Widget uiDialogButton(UiDialogAction action) {
  final shape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(UiSpace.buttonRadius));
  final Widget button;
  switch (action.kind) {
    case UiDialogActionKind.primary:
      button = ElevatedButton(
          style: ElevatedButton.styleFrom(
              backgroundColor: UiColor.primary,
              foregroundColor: Colors.white,
              disabledBackgroundColor: UiColor.primaryDisabled,
              disabledForegroundColor: Colors.white,
              elevation: 0,
              padding: const EdgeInsets.symmetric(horizontal: UiSpace.s4),
              shape: shape,
              textStyle: UiType.button),
          onPressed: action.onPressed,
          child: Text(translate(action.label)));
      break;
    case UiDialogActionKind.secondary:
      button = OutlinedButton(
          style: OutlinedButton.styleFrom(
              foregroundColor: UiColor.text,
              backgroundColor: Colors.white,
              side: const BorderSide(color: UiColor.inputBorder),
              padding: const EdgeInsets.symmetric(horizontal: UiSpace.s3),
              shape: shape,
              textStyle: UiType.button),
          onPressed: action.onPressed,
          child: Text(translate(action.label)));
      break;
    case UiDialogActionKind.danger:
      button = OutlinedButton(
          style: OutlinedButton.styleFrom(
              foregroundColor: UiColor.danger,
              backgroundColor: Colors.white,
              side: const BorderSide(color: UiColor.dangerBorder),
              padding: const EdgeInsets.symmetric(horizontal: UiSpace.s3),
              shape: shape,
              textStyle: UiType.button),
          onPressed: action.onPressed,
          child: Text(translate(action.label)));
      break;
  }
  return SizedBox(height: UiSpace.controlHeight, child: button);
}
