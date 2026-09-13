
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../../../models/platform_model.dart';
import '../../../desktop/widgets/ui_tokens.dart';
import '../ui_dialog.dart';
import '../ui_fields.dart';
import 'validation.dart';
import 'package:flutter_hbb/models/model.dart';

void wrongPasswordDialog(SessionID sessionId,
    OverlayDialogManager dialogManager, type, title, text) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      enterPasswordDialog(sessionId, dialogManager);
    }

    return UiDialog(
      title: translate(title),
      onClose: cancel,
      body: uiDialogText(translate(text)),
      actions: [
        UiDialogAction.secondary('Cancel', cancel),
        UiDialogAction.primary('Retry', submit),
      ],
    ).alert(context);
  });
}

void enterPasswordDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  await _connectDialog(
    sessionId,
    dialogManager,
    passwordController: TextEditingController(),
  );
}

void enterUserLoginDialog(SessionID sessionId,
    OverlayDialogManager dialogManager, String osAccountDescTip) async {
  await _connectDialog(
    sessionId,
    dialogManager,
    osUsernameController: TextEditingController(),
    osPasswordController: TextEditingController(),
    osAccountDescTip: osAccountDescTip,
  );
}

void enterUserLoginAndPasswordDialog(SessionID sessionId,
    OverlayDialogManager dialogManager, String osAccountDescTip) async {
  await _connectDialog(
    sessionId,
    dialogManager,
    osUsernameController: TextEditingController(),
    osPasswordController: TextEditingController(),
    passwordController: TextEditingController(),
    osAccountDescTip: osAccountDescTip,
  );
}

_connectDialog(
  SessionID sessionId,
  OverlayDialogManager dialogManager, {
  TextEditingController? osUsernameController,
  TextEditingController? osPasswordController,
  TextEditingController? passwordController,
  String? osAccountDescTip,
}) async {
  final errUsername = ''.obs;
  var rememberPassword = false;
  var showPassword = false;
  if (passwordController != null) {
    rememberPassword =
        await bind.sessionGetRemember(sessionId: sessionId) ?? false;
  }
  if (osUsernameController != null) {
    osUsernameController.addListener(() {
      if (errUsername.value.isNotEmpty) {
        errUsername.value = '';
      }
    });
  }

  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      if (osUsernameController != null) {
        if (osUsernameController.text.trim().isEmpty) {
          errUsername.value = translate('Empty Username');
          setState(() {});
          return;
        }
      }
      final osUsername = osUsernameController?.text.trim() ?? '';
      final osPassword = osPasswordController?.text.trim() ?? '';
      final password = passwordController?.text.trim() ?? '';
      if (passwordController != null && password.isEmpty) return;
      gFFI.login(
        osUsername,
        osPassword,
        sessionId,
        password,
        rememberPassword,
      );
      close();
      dialogManager.showLoading(translate('Logging in...'),
          onCancel: closeConnection);
    }

    toggleShow() => setState(() => showPassword = !showPassword);

    osAccountWidget() {
      if (osUsernameController == null || osPasswordController == null) {
        return Offstage();
      }
      return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        if (osAccountDescTip != null) ...[
          uiDialogText(translate(osAccountDescTip)),
          const SizedBox(height: UiSpace.s3),
        ],
        uiDialogField(
            translate(DialogTextField.kUsernameTitle), osUsernameController,
            error: errUsername.value, autoFocus: true, onSubmitted: submit),
        uiDialogField(translate('Password'), osPasswordController,
            obscure: !showPassword,
            onToggleObscure: toggleShow,
            onSubmitted: submit),
      ]);
    }

    passwdWidget() {
      if (passwordController == null) {
        return Offstage();
      }
      return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        uiDialogText(translate('verify_rustdesk_password_tip')),
        const SizedBox(height: UiSpace.s3),
        uiDialogField(translate('Password'), passwordController,
            autoFocus: osUsernameController == null,
            obscure: !showPassword,
            onToggleObscure: toggleShow,
            onSubmitted: submit),
        uiDialogToggle(translate('Remember password'), rememberPassword,
            (v) => setState(() => rememberPassword = v)),
      ]);
    }

    return UiDialog(
      title: translate('Password Required'),
      onClose: cancel,
      body: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            osAccountWidget(),
            if (osUsernameController != null && passwordController != null)
              const SizedBox(height: UiSpace.s3),
            passwdWidget(),
          ]),
      actions: [
        UiDialogAction.secondary('Cancel', cancel),
        UiDialogAction.primary('OK', submit),
      ],
    ).alert(context);
  });
}
