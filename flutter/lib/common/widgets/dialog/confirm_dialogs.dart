
import 'package:flutter/material.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:get/get.dart';

import '../../../common.dart';

void deleteConfirmDialog(Function onSubmit, String title) async {
  gFFI.dialogManager.show(
    (setState, close, context) {
      submit() async {
        await onSubmit();
        close();
      }

      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(
              Icons.delete_rounded,
              color: Colors.red,
            ),
            Expanded(
              child: Text(title, overflow: TextOverflow.ellipsis).paddingOnly(
                left: 10,
              ),
            ),
          ],
        ),
        content: SizedBox.shrink(),
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
            onPressed: submit,
          ),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    },
  );
}

void renameDialog(
    {required String oldName,
    FormFieldValidator<String>? validator,
    required ValueChanged<String> onSubmit,
    Function? onCancel}) async {
  RxBool isInProgress = false.obs;
  var controller = TextEditingController(text: oldName);
  final formKey = GlobalKey<FormState>();
  gFFI.dialogManager.show((setState, close, context) {
    submit() async {
      String text = controller.text.trim();
      if (validator != null && formKey.currentState?.validate() == false) {
        return;
      }
      isInProgress.value = true;
      onSubmit(text);
      close();
      isInProgress.value = false;
    }

    cancel() {
      onCancel?.call();
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.edit_rounded, color: MyTheme.accent),
          Text(translate('Rename')).paddingOnly(left: 10),
        ],
      ),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            child: Form(
              key: formKey,
              child: TextFormField(
                controller: controller,
                autofocus: true,
                decoration: InputDecoration(labelText: translate('Name')),
                validator: validator,
              ).workaroundFreezeLinuxMint(),
            ),
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          Obx(() =>
              isInProgress.value ? const LinearProgressIndicator() : Offstage())
        ],
      ),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onSubmit: submit,
      onCancel: cancel,
    );
  });
}

void setSharedAbPasswordDialog(String abName, Peer peer) {
  TextEditingController controller = TextEditingController(text: '');
  RxBool isInProgress = false.obs;
  RxBool isInputEmpty = true.obs;
  bool passwordVisible = false;
  controller.addListener(() {
    isInputEmpty.value = controller.text.isEmpty;
  });
  gFFI.dialogManager.show((setState, close, context) {
    change(String password) async {
      isInProgress.value = true;
      bool res =
          await gFFI.abModel.changeSharedPassword(abName, peer.id, password);
      isInProgress.value = false;
      if (res) {
        showToast(translate('Successful'));
      }
      close();
    }

    cancel() {
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.key, color: MyTheme.accent),
          Text(translate(peer.password.isEmpty
                  ? 'Set shared password'
                  : 'Change Password'))
              .paddingOnly(left: 10),
        ],
      ),
      content: Obx(() => Column(children: [
            TextField(
              controller: controller,
              autofocus: true,
              obscureText: !passwordVisible,
              decoration: InputDecoration(
                suffixIcon: IconButton(
                  icon: Icon(
                      passwordVisible ? Icons.visibility : Icons.visibility_off,
                      color: MyTheme.lightTheme.primaryColor),
                  onPressed: () {
                    setState(() {
                      passwordVisible = !passwordVisible;
                    });
                  },
                ),
              ),
            ).workaroundFreezeLinuxMint(),
            if (!gFFI.abModel.current.isPersonal())
              Row(children: [
                Icon(Icons.info, color: Colors.amber).marginOnly(right: 4),
                Text(
                  translate('share_warning_tip'),
                  style: TextStyle(fontSize: 12),
                )
              ]).marginSymmetric(vertical: 10),
            // NOT use Offstage to wrap LinearProgressIndicator
            isInProgress.value ? const LinearProgressIndicator() : Offstage()
          ])),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: cancel,
          isOutline: true,
        ),
        if (peer.password.isNotEmpty)
          dialogButton(
            "Remove",
            icon: Icon(Icons.delete_outline_rounded),
            onPressed: () => change(''),
            buttonStyle: ButtonStyle(
                backgroundColor: MaterialStatePropertyAll(Colors.red)),
          ),
        Obx(() => dialogButton(
              "OK",
              icon: Icon(Icons.done_rounded),
              onPressed:
                  isInputEmpty.value ? null : () => change(controller.text),
            )),
      ],
      onSubmit: isInputEmpty.value ? null : () => change(controller.text),
      onCancel: cancel,
    );
  });
}

void CommonConfirmDialog(OverlayDialogManager dialogManager, String content,
    VoidCallback onConfirm) {
  dialogManager.show((setState, close, context) {
    submit() {
      close();
      onConfirm.call();
    }

    return CustomAlertDialog(
      content: Row(
        children: [
          Expanded(
            child: Text(content,
                style: const TextStyle(fontSize: 15),
                textAlign: TextAlign.start),
          ),
        ],
      ).marginOnly(bottom: 12),
      actions: [
        dialogButton(translate("Cancel"), onPressed: close, isOutline: true),
        dialogButton(translate("OK"), onPressed: submit),
      ],
      onSubmit: submit,
      onCancel: close,
    );
  });
}
