
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../../../models/platform_model.dart';
import '../ui_dialog.dart';
import '../ui_fields.dart';
import 'validation.dart';

void showWaitUacDialog(
    SessionID sessionId, OverlayDialogManager dialogManager, String type) {
  dialogManager.dismissAll();
  dialogManager.show(
      tag: '$sessionId-wait-uac',
      (setState, close, context) => CustomAlertDialog(
            title: null,
            content: msgboxContent(type, 'Wait', 'wait_accept_uac_tip'),
            actions: [
              dialogButton(
                'OK',
                icon: Icon(Icons.done_rounded),
                onPressed: close,
              ),
            ],
          ));
}

// Another username && password dialog?
void showRequestElevationDialog(
    SessionID sessionId, OverlayDialogManager dialogManager) {
  RxString groupValue = ''.obs;
  RxString errUser = ''.obs;
  RxString errPwd = ''.obs;
  TextEditingController userController = TextEditingController();
  TextEditingController pwdController = TextEditingController();

  void onRadioChanged(String? value) {
    if (value != null) {
      groupValue.value = value;
    }
  }

  // TODO get from theme
  final double fontSizeNote = 13.00;

  Widget OptionRequestPermissions = Obx(
    () => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Radio(
          visualDensity: VisualDensity(horizontal: -4, vertical: -4),
          value: '',
          groupValue: groupValue.value,
          onChanged: onRadioChanged,
        ).marginOnly(right: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              InkWell(
                hoverColor: Colors.transparent,
                onTap: () => groupValue.value = '',
                child: Text(
                  translate('Ask the remote user for authentication'),
                ),
              ).marginOnly(bottom: 10),
              Text(
                translate('Choose this if the remote account is administrator'),
                style: TextStyle(fontSize: fontSizeNote),
              ),
            ],
          ).marginOnly(top: 3),
        ),
      ],
    ),
  );

  Widget OptionCredentials = Obx(
    () => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Radio(
          visualDensity: VisualDensity(horizontal: -4, vertical: -4),
          value: 'logon',
          groupValue: groupValue.value,
          onChanged: onRadioChanged,
        ).marginOnly(right: 10),
        Expanded(
          child: InkWell(
            hoverColor: Colors.transparent,
            onTap: () => onRadioChanged('logon'),
            child: Text(
              translate('Transmit the username and password of administrator'),
            ),
          ).marginOnly(top: 4),
        ),
      ],
    ),
  );

  Widget UacNote = Container(
    padding: EdgeInsets.fromLTRB(10, 8, 8, 8),
    decoration: BoxDecoration(
      color: MyTheme.currentThemeMode() == ThemeMode.dark
          ? Color.fromARGB(135, 87, 87, 90)
          : Colors.grey[100],
      borderRadius: BorderRadius.circular(8),
      border: Border.all(color: Colors.grey),
    ),
    child: Row(
      children: [
        Icon(Icons.info_outline_rounded, size: 20).marginOnly(right: 10),
        Expanded(
          child: Text(
            translate('still_click_uac_tip'),
            style: TextStyle(
                fontSize: fontSizeNote, fontWeight: FontWeight.normal),
          ),
        )
      ],
    ),
  );

  var content = Obx(
    () => Column(
      children: [
        OptionRequestPermissions.marginOnly(bottom: 15),
        OptionCredentials,
        Offstage(
          offstage: 'logon' != groupValue.value,
          child: Column(
            children: [
              UacNote.marginOnly(bottom: 10),
              DialogTextField(
                controller: userController,
                title: translate('Username'),
                hintText: translate('elevation_username_tip'),
                prefixIcon: DialogTextField.kUsernameIcon,
                errorText: errUser.isEmpty ? null : errUser.value,
              ),
              PasswordWidget(
                controller: pwdController,
                autoFocus: false,
                errorText: errPwd.isEmpty ? null : errPwd.value,
              ),
            ],
          ).marginOnly(left: stateGlobal.isPortrait.isFalse ? 35 : 0),
        ).marginOnly(top: 10),
      ],
    ),
  );

  dialogManager.dismissAll();
  dialogManager.show(tag: '$sessionId-request-elevation',
      (setState, close, context) {
    void submit() {
      if (groupValue.value == 'logon') {
        if (userController.text.isEmpty) {
          errUser.value = translate('Empty Username');
          return;
        }
        if (pwdController.text.isEmpty) {
          errPwd.value = translate('Empty Password');
          return;
        }
        bind.sessionElevateWithLogon(
            sessionId: sessionId,
            username: userController.text,
            password: pwdController.text);
      } else {
        bind.sessionElevateDirect(sessionId: sessionId);
      }
      close();
      showWaitUacDialog(sessionId, dialogManager, "wait-uac");
    }

    return CustomAlertDialog(
      title: Text(translate('Request Elevation')),
      content: content,
      actions: [
        dialogButton(
          'Cancel',
          icon: Icon(Icons.close_rounded),
          onPressed: close,
          isOutline: true,
        ),
        dialogButton(
          'OK',
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        )
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showOnBlockDialog(
  SessionID sessionId,
  String type,
  String title,
  String text,
  OverlayDialogManager dialogManager,
) {
  if (dialogManager.existing('$sessionId-wait-uac') ||
      dialogManager.existing('$sessionId-request-elevation')) {
    return;
  }
  dialogManager.show(tag: '$sessionId-$type', (setState, close, context) {
    void submit() {
      close();
      showRequestElevationDialog(sessionId, dialogManager);
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title,
          "${translate(text)}${type.contains('uac') ? '\n' : '\n\n'}${translate('request_elevation_tip')}"),
      actions: [
        dialogButton('Wait', onPressed: close, isOutline: true),
        dialogButton('Request Elevation', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showElevationError(SessionID sessionId, String type, String title,
    String text, OverlayDialogManager dialogManager) {
  dialogManager.show(tag: '$sessionId-$type', (setState, close, context) {
    void submit() {
      close();
      showRequestElevationDialog(sessionId, dialogManager);
    }

    return CustomAlertDialog(
      title: null,
      content: msgboxContent(type, title, text),
      actions: [
        dialogButton('Cancel', onPressed: () {
          close();
        }, isOutline: true),
        if (text != 'No permission') dialogButton('Retry', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

void showWaitAcceptDialog(SessionID sessionId, String type, String title,
    String text, OverlayDialogManager dialogManager) {
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    onCancel() {
      closeConnection();
    }

    return UiDialog(
      title: translate(title),
      onClose: onCancel,
      body: uiDialogText(translate(text)),
      actions: [
        UiDialogAction.secondary('Cancel', onCancel),
      ],
    ).alert(context);
  });
}
