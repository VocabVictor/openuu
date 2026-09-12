import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '../../../models/model.dart';
import '../../../models/platform_model.dart';
import 'validation.dart';

void showRestartRemoteDevice(PeerInfo pi, String id, SessionID sessionId,
    OverlayDialogManager dialogManager) async {
  final res = await dialogManager
      .show<bool>((setState, close, context) => CustomAlertDialog(
            title: Row(children: [
              Icon(Icons.warning_rounded, color: Colors.redAccent, size: 28),
              Flexible(
                  child: Text(translate("Restart remote device"))
                      .paddingOnly(left: 10)),
            ]),
            content: Text(
                "${translate('Are you sure you want to restart')} \n${pi.username}@${pi.hostname}($id) ?"),
            actions: [
              dialogButton(
                "Cancel",
                icon: Icon(Icons.close_rounded),
                onPressed: close,
                isOutline: true,
              ),
              dialogButton(
                "OK",
                icon: Icon(Icons.done_rounded),
                onPressed: () => close(true),
              ),
            ],
            onCancel: close,
            onSubmit: () => close(true),
          ));
  if (res == true) bind.sessionRestartRemoteDevice(sessionId: sessionId);
}

showSetOSPassword(
  SessionID sessionId,
  bool login,
  OverlayDialogManager dialogManager,
  String? osPassword,
  Function()? closeCallback,
) async {
  final controller = TextEditingController();
  osPassword ??=
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-password') ??
          '';
  var autoLogin =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'auto-login') !=
          '';
  controller.text = osPassword;
  dialogManager.show((setState, close, context) {
    closeWithCallback([dynamic]) {
      close();
      if (closeCallback != null) closeCallback();
    }

    submit() {
      var text = controller.text.trim();
      bind.sessionPeerOption(
          sessionId: sessionId, name: 'os-password', value: text);
      bind.sessionPeerOption(
          sessionId: sessionId,
          name: 'auto-login',
          value: autoLogin ? 'Y' : '');
      if (text != '' && login) {
        bind.sessionInputOsPassword(sessionId: sessionId, value: text);
      }
      closeWithCallback();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.password_rounded, color: MyTheme.accent),
          Text(translate('OS Password')).paddingOnly(left: 10),
        ],
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          PasswordWidget(controller: controller),
          CheckboxListTile(
            contentPadding: const EdgeInsets.all(0),
            dense: true,
            controlAffinity: ListTileControlAffinity.leading,
            title: Text(
              translate('Auto Login'),
            ),
            value: autoLogin,
            onChanged: (v) {
              if (v == null) return;
              setState(() => autoLogin = v);
            },
          ),
        ],
      ),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: closeWithCallback,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: closeWithCallback,
    );
  });
}

Widget buildNoteTextField({
  required TextEditingController controller,
  required VoidCallback onEscape,
}) {
  final focusNode = FocusNode(
    onKey: (FocusNode node, RawKeyEvent evt) {
      if (evt.logicalKey.keyLabel == 'Enter') {
        if (evt is RawKeyDownEvent) {
          int pos = controller.selection.base.offset;
          controller.text =
              '${controller.text.substring(0, pos)}\n${controller.text.substring(pos)}';
          controller.selection =
              TextSelection.fromPosition(TextPosition(offset: pos + 1));
        }
        return KeyEventResult.handled;
      }
      if (evt.logicalKey.keyLabel == 'Esc') {
        if (evt is RawKeyDownEvent) {
          onEscape();
        }
        return KeyEventResult.handled;
      } else {
        return KeyEventResult.ignored;
      }
    },
  );

  return TextField(
    autofocus: true,
    keyboardType: TextInputType.multiline,
    textInputAction: TextInputAction.newline,
    decoration: InputDecoration(
      hintText: translate('input note here'),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(8),
      ),
      contentPadding: EdgeInsets.all(12),
    ),
    minLines: 5,
    maxLines: null,
    maxLength: 256,
    controller: controller,
    focusNode: focusNode,
  ).workaroundFreezeLinuxMint();
}

void showConfirmSwitchSidesDialog(
    SessionID sessionId, String id, OverlayDialogManager dialogManager) async {
  dialogManager.show((setState, close, context) {
    submit() async {
      await bind.sessionSwitchSides(sessionId: sessionId);
      closeConnection(id: id);
    }

    return CustomAlertDialog(
      content: msgboxContent('info', 'Switch Sides',
          'Please confirm if you want to share your desktop?'),
      actions: [
        dialogButton('Cancel', onPressed: close, isOutline: true),
        dialogButton('OK', onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}

// This dialog should not be dismissed, otherwise it will be black screen, have not reproduced this.
void showWindowsSessionsDialog(
    String type,
    String title,
    String text,
    OverlayDialogManager dialogManager,
    SessionID sessionId,
    String peerId,
    String sessions) {
  List<dynamic> sessionsList = [];
  try {
    sessionsList = json.decode(sessions);
  } catch (e) {
    print(e);
  }
  List<String> sids = [];
  List<String> names = [];
  for (var session in sessionsList) {
    sids.add(session['sid']);
    names.add(session['name']);
  }
  String selectedUserValue = sids.first;
  dialogManager.dismissAll();
  dialogManager.show((setState, close, context) {
    submit() {
      bind.sessionSendSelectedSessionId(
          sessionId: sessionId, sid: selectedUserValue);
      close();
    }

    return CustomAlertDialog(
      title: null,
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          msgboxContent(type, title, text).marginOnly(bottom: 12),
          ComboBox(
              keys: sids,
              values: names,
              initialKey: selectedUserValue,
              onChanged: (value) {
                selectedUserValue = value;
              }),
        ],
      ),
      actions: [
        dialogButton('Connect', onPressed: submit, isOutline: false),
      ],
    );
  });
}
