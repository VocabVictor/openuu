part of 'toolbar.dart';

final Set<String> _waylandKeyboardPromptSuppressedConnectionIds = <String>{};

Future<bool> openWaylandKeyboardIssueUrl() {
  return launchUrl(
    Uri.parse(kWaylandKeyboardIssueUrl),
    mode: LaunchMode.externalApplication,
  );
}

bool isWaylandKeyboardPromptSuppressedForConnection(String connectionId) {
  return _waylandKeyboardPromptSuppressedConnectionIds.contains(connectionId);
}

void setWaylandKeyboardPromptSuppressedForConnection(
    String connectionId, bool suppressed) {
  if (suppressed) {
    _waylandKeyboardPromptSuppressedConnectionIds.add(connectionId);
  } else {
    _waylandKeyboardPromptSuppressedConnectionIds.remove(connectionId);
  }
}

void clearWaylandKeyboardPromptSuppressedForConnection(String connectionId) {
  _waylandKeyboardPromptSuppressedConnectionIds.remove(connectionId);
}

bool shouldShowWaylandKeyboardPrompt({
  required String connectionId,
  required bool isWaylandPeer,
  required bool allowWaylandKeyboardRemembered,
}) {
  return isWaylandPeer &&
      !allowWaylandKeyboardRemembered &&
      !isWaylandKeyboardPromptSuppressedForConnection(connectionId);
}

Widget waylandKeyboardScopeChip(BuildContext context, String text) {
  final colorScheme = Theme.of(context).colorScheme;
  return Container(
    padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
    decoration: BoxDecoration(
      borderRadius: BorderRadius.circular(999),
      border: Border.all(color: colorScheme.primary.withOpacity(0.35)),
    ),
    child: Text(
      text,
      style: Theme.of(
        context,
      ).textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w600),
    ),
  );
}

void showWaylandKeyboardInputWarningDialog(
    {required String id,
    required String connectionId,
    required FFI ffi,
    required Future<void> Function() onEnable}) {
  bool remember = false;
  bool consentInProgress = false;
  bool dialogClosed = false;

  final dialogFuture = ffi.dialogManager.show((setState, close, context) {
    void safeSetState(VoidCallback fn) {
      if (dialogClosed) {
        return;
      }
      try {
        setState(fn);
      } catch (e) {
        debugPrint('Ignore setState after dialog disposal: $e');
      }
    }

    void closeDialog() {
      if (dialogClosed) {
        return;
      }
      dialogClosed = true;
      close();
    }

    Future<void> enableAndContinue() async {
      if (consentInProgress || dialogClosed) {
        return;
      }
      consentInProgress = true;
      safeSetState(() {});
      try {
        await onEnable();
      } catch (e, st) {
        debugPrint('Failed to enable Wayland keyboard input consent: $e');
        debugPrintStack(stackTrace: st);
        consentInProgress = false;
        safeSetState(() {});
        return;
      }

      ffi.inputModel.keyboardInputAllowed = true;
      var rememberPersisted = true;
      if (remember) {
        try {
          await bind.mainSetPeerOption(
              id: id,
              key: kPeerOptionAllowWaylandKeyboard,
              value: bool2option(kPeerOptionAllowWaylandKeyboard, true));
        } catch (e) {
          rememberPersisted = false;
          debugPrint('Failed to persist Wayland keyboard input consent: $e');
        }
      }
      // Always suppress prompt for current connection after explicit consent.
      setWaylandKeyboardPromptSuppressedForConnection(connectionId, true);
      closeDialog();
      if (remember && !rememberPersisted) {
        // It's a rare edge case that persisting the user's choice fails.
        // Failed to persist the user's choice, but still allow keyboard input for current session.
        showToast(translate('Failed'));
      }
    }

    void cancel() {
      if (consentInProgress) {
        return;
      }
      closeDialog();
    }

    return CustomAlertDialog(
      title: null,
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          msgboxContent(
            '',
            'wayland-keyboard-input-disabled-tip',
            'wayland-keyboard-input-consent-tip',
          ),
          SizedBox(height: isMobile ? 2 : 6),
          if (isMobile) ...[
            Text(
              translate('wayland-keyboard-input-applies-to-tip'),
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
            ).marginOnly(bottom: 6),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: [
                waylandKeyboardScopeChip(
                    context, translate('Send clipboard keystrokes')),
                waylandKeyboardScopeChip(
                    context, translate('wayland-soft-keyboard-input-label')),
              ],
            ).marginOnly(bottom: 10),
          ],
          TextButton(
            onPressed: consentInProgress
                ? null
                : () async {
                    try {
                      final opened = await openWaylandKeyboardIssueUrl();
                      if (!opened) {
                        // Opening this optional help link almost never fails in
                        // normal desktop environments. Keep the result handled
                        // for review hygiene, but avoid a low-value user toast.
                        debugPrint('Failed to open Wayland keyboard issue URL');
                      }
                    } catch (e) {
                      debugPrint(
                          'Failed to open Wayland keyboard issue URL: $e');
                    }
                  },
            style: TextButton.styleFrom(
              foregroundColor: Colors.blue,
              padding: EdgeInsets.zero,
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            child: Text(
              translate('Why this happens'),
              style: const TextStyle(decoration: TextDecoration.underline),
            ),
          ).marginOnly(bottom: 6),
          CheckboxListTile(
            value: remember,
            dense: true,
            contentPadding: EdgeInsets.zero,
            controlAffinity: ListTileControlAffinity.leading,
            title: Text(translate('remember-wayland-keyboard-choice-tip')),
            onChanged: consentInProgress
                ? null
                : (v) {
                    safeSetState(() => remember = v == true);
                  },
          ),
        ],
      ),
      actions: [
        dialogButton(
          'Cancel',
          onPressed: consentInProgress ? null : cancel,
          isOutline: true,
        ),
        dialogButton(
          'OK',
          onPressed:
              consentInProgress ? null : () => unawaited(enableAndContinue()),
        ),
      ],
      onCancel: consentInProgress ? null : cancel,
      onSubmit: consentInProgress ? null : () => unawaited(enableAndContinue()),
    );
  }, clickMaskDismiss: false, backDismiss: false);
  unawaited(dialogFuture.whenComplete(() => dialogClosed = true));
}
