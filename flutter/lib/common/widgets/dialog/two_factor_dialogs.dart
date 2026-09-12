
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../../../common.dart';
import '../../../models/platform_model.dart';
import 'code_fields.dart';

void changeBot({Function()? callback}) async {
  if (bind.mainHasValidBotSync()) {
    await bind.mainSetOption(key: "bot", value: "");
    callback?.call();
    return;
  }
  String errorText = '';
  bool loading = false;
  final controller = TextEditingController();
  gFFI.dialogManager.show((setState, close, context) {
    onVerify() async {
      final token = controller.text.trim();
      if (token == "") return;
      loading = true;
      errorText = '';
      setState(() {});
      final error = await bind.mainVerifyBot(token: token);
      if (error == "") {
        callback?.call();
        close();
      } else {
        errorText = translate(error);
        loading = false;
        setState(() {});
      }
    }

    final codeField = TextField(
      autofocus: true,
      controller: controller,
      decoration: InputDecoration(
        hintText: translate('Token'),
      ),
    ).workaroundFreezeLinuxMint();

    return CustomAlertDialog(
      title: Text(translate("Telegram bot")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SelectableText(translate("enable-bot-desc"),
                  style: TextStyle(fontSize: 12))
              .marginOnly(bottom: 12),
          Row(children: [Expanded(child: codeField)]),
          if (errorText != '')
            Text(errorText, style: TextStyle(color: Colors.red))
                .marginOnly(top: 12),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        loading
            ? CircularProgressIndicator()
            : dialogButton("OK", onPressed: onVerify),
      ],
      onCancel: close,
    );
  });
}

void change2fa({Function()? callback}) async {
  if (bind.mainHasValid2FaSync()) {
    await bind.mainSetOption(key: "2fa", value: "");
    await bind.mainClearTrustedDevices();
    callback?.call();
    return;
  }
  var new2fa = (await bind.mainGenerate2Fa());
  final secretRegex = RegExp(r'secret=([^&]+)');
  final secret = secretRegex.firstMatch(new2fa)?.group(1);
  String? errorText;
  final controller = TextEditingController();
  gFFI.dialogManager.show((setState, close, context) {
    onVerify() async {
      if (await bind.mainVerify2Fa(code: controller.text.trim())) {
        callback?.call();
        close();
      } else {
        errorText = translate('wrong-2fa-code');
      }
    }

    final codeField = Dialog2FaField(
      controller: controller,
      errorText: errorText,
      onChanged: () => setState(() => errorText = null),
      title: translate('Verification code'),
      readyCallback: () {
        onVerify();
        setState(() {});
      },
    );

    getOnSubmit() => codeField.isReady ? onVerify : null;

    return CustomAlertDialog(
      title: Text(translate("enable-2fa-title")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SelectableText(translate("enable-2fa-desc"),
                  style: TextStyle(fontSize: 12))
              .marginOnly(bottom: 12),
          SizedBox(
              width: 160,
              height: 160,
              child: QrImageView(
                backgroundColor: Colors.white,
                data: new2fa,
                version: QrVersions.auto,
                size: 160,
                gapless: false,
              )).marginOnly(bottom: 6),
          SelectableText(secret ?? '', style: TextStyle(fontSize: 12))
              .marginOnly(bottom: 12),
          Row(children: [Expanded(child: codeField)]),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: getOnSubmit()),
      ],
      onCancel: close,
    );
  });
}

void enter2FaDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  final controller = TextEditingController();
  final RxBool submitReady = false.obs;
  final RxBool trustThisDevice = false.obs;

  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    cancel() {
      close();
      closeConnection();
    }

    submit() {
      gFFI.send2FA(sessionId, controller.text.trim(), trustThisDevice.value);
      close();
      dialogManager.showLoading(translate('Logging in...'),
          onCancel: closeConnection);
    }

    late Dialog2FaField codeField;

    codeField = Dialog2FaField(
      controller: controller,
      title: translate('Verification code'),
      onChanged: () => submitReady.value = codeField.isReady,
    );

    final trustField = Obx(() => CheckboxListTile(
          contentPadding: const EdgeInsets.all(0),
          dense: true,
          controlAffinity: ListTileControlAffinity.leading,
          title: Text(translate("Trust this device")),
          value: trustThisDevice.value,
          onChanged: (value) {
            if (value == null) return;
            trustThisDevice.value = value;
          },
        ));

    return CustomAlertDialog(
        title: Text(translate('enter-2fa-title')),
        content: Column(
          children: [
            codeField,
            if (bind.sessionGetEnableTrustedDevices(sessionId: sessionId))
              trustField,
          ],
        ),
        actions: [
          dialogButton('Cancel',
              onPressed: cancel,
              isOutline: true,
              style: TextStyle(
                  color: Theme.of(context).textTheme.bodyMedium?.color)),
          Obx(() => dialogButton(
                'OK',
                onPressed: submitReady.isTrue ? submit : null,
              )),
        ],
        onSubmit: submit,
        onCancel: cancel);
  });
}
