
import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../../../models/platform_model.dart';
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

    return CustomAlertDialog(
        title: null,
        content: msgboxContent(type, title, text),
        onSubmit: submit,
        onCancel: cancel,
        actions: [
          dialogButton(
            'Cancel',
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            'Retry',
            onPressed: submit,
          ),
        ]);
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

    descWidget(String text) {
      return Column(
        children: [
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              text,
              maxLines: 3,
              softWrap: true,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(fontSize: 16),
            ),
          ),
          Container(
            height: 8,
          ),
        ],
      );
    }

    rememberWidget(
      String desc,
      bool remember,
      ValueChanged<bool?>? onChanged,
    ) {
      return CheckboxListTile(
        contentPadding: const EdgeInsets.all(0),
        dense: true,
        controlAffinity: ListTileControlAffinity.leading,
        title: Text(desc),
        value: remember,
        onChanged: onChanged,
      );
    }

    osAccountWidget() {
      if (osUsernameController == null || osPasswordController == null) {
        return Offstage();
      }
      return Column(
        children: [
          if (osAccountDescTip != null) descWidget(translate(osAccountDescTip)),
          DialogTextField(
            title: translate(DialogTextField.kUsernameTitle),
            controller: osUsernameController,
            prefixIcon: DialogTextField.kUsernameIcon,
            errorText: null,
          ),
          if (errUsername.value.isNotEmpty)
            Align(
              alignment: Alignment.centerLeft,
              child: SelectableText(
                errUsername.value,
                style: TextStyle(
                  color: Theme.of(context).colorScheme.error,
                  fontSize: 12,
                ),
                textAlign: TextAlign.left,
              ).paddingOnly(left: 12, bottom: 2),
            ),
          PasswordWidget(
            controller: osPasswordController,
            autoFocus: false,
          ),
        ],
      );
    }

    passwdWidget() {
      if (passwordController == null) {
        return Offstage();
      }
      return Column(
        children: [
          descWidget(translate('verify_rustdesk_password_tip')),
          PasswordWidget(
            controller: passwordController,
            autoFocus: osUsernameController == null,
          ),
          rememberWidget(
            translate('Remember password'),
            rememberPassword,
            (v) {
              if (v != null) {
                setState(() => rememberPassword = v);
              }
            },
          ),
        ],
      );
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.password_rounded, color: MyTheme.accent),
          Text(translate('Password Required')).paddingOnly(left: 10),
        ],
      ),
      content: Column(mainAxisSize: MainAxisSize.min, children: [
        osAccountWidget(),
        osUsernameController == null || passwordController == null
            ? Offstage()
            : Container(height: 12),
        passwdWidget(),
      ]),
      actions: [
        dialogButton(
          'Cancel',
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        dialogButton(
          'OK',
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}
