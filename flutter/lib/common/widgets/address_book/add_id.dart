part of 'address_book.dart';

extension _AddressBookAddId on _AddressBookState {
  void addIdToCurrentAb() async {
    if (gFFI.abModel.isCurrentAbFull(true)) {
      return;
    }
    var isInProgress = false;
    var passwordVisible = false;
    IDTextEditingController idController = IDTextEditingController(text: '');
    TextEditingController aliasController = TextEditingController(text: '');
    TextEditingController passwordController = TextEditingController(text: '');
    TextEditingController noteController = TextEditingController(text: '');
    final tags = List.of(gFFI.abModel.currentAbTags);
    var selectedTag = List<dynamic>.empty(growable: true).obs;
    final style = TextStyle(fontSize: 14.0);
    String? errorMsg;
    final isCurrentAbShared = !gFFI.abModel.current.isPersonal();

    gFFI.dialogManager.show((setState, close, context) {
      submit() async {
        setState(() {
          isInProgress = true;
          errorMsg = null;
        });
        String id = idController.id;
        if (id.isEmpty) {
          // pass
        } else {
          if (gFFI.abModel.idContainByCurrent(id)) {
            setState(() {
              isInProgress = false;
              errorMsg = translate('ID already exists');
            });
            return;
          }
          var password = '';
          if (isCurrentAbShared) {
            password = passwordController.text;
          }
          String? errMsg2 = await gFFI.abModel.addIdToCurrent(
              id,
              aliasController.text.trim(),
              password,
              selectedTag,
              noteController.text);
          if (errMsg2 != null) {
            setState(() {
              isInProgress = false;
              errorMsg = errMsg2;
            });
            return;
          }
          // final currentPeers
        }
        close();
      }

      double marginBottom = 4;

      row({required Widget label, required Widget input}) {
        makeChild(bool isPortrait) => Row(
              children: [
                !isPortrait
                    ? ConstrainedBox(
                        constraints: const BoxConstraints(minWidth: 100),
                        child: label.marginOnly(right: 10))
                    : SizedBox.shrink(),
                Expanded(
                  child: ConstrainedBox(
                      constraints: const BoxConstraints(minWidth: 200),
                      child: input),
                ),
              ],
            ).marginOnly(bottom: !isPortrait ? 8 : 0);
        return Obx(() => makeChild(stateGlobal.isPortrait.isTrue));
      }

      return CustomAlertDialog(
        title: Text(translate("Add ID")),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Column(
              children: [
                row(
                    label: Row(
                      children: [
                        Text(
                          '*',
                          style: TextStyle(color: Colors.red, fontSize: 14),
                        ),
                        Text(
                          'ID',
                          style: style,
                        ),
                      ],
                    ),
                    input: Obx(() => TextField(
                          controller: idController,
                          inputFormatters: [IDTextInputFormatter()],
                          decoration: InputDecoration(
                              labelText: stateGlobal.isPortrait.isFalse
                                  ? null
                                  : translate('ID'),
                              errorText: errorMsg,
                              errorMaxLines: 5),
                        ).workaroundFreezeLinuxMint())),
                row(
                  label: Text(
                    translate('Alias'),
                    style: style,
                  ),
                  input: Obx(() => TextField(
                        controller: aliasController,
                        decoration: InputDecoration(
                          labelText: stateGlobal.isPortrait.isFalse
                              ? null
                              : translate('Alias'),
                        ),
                      ).workaroundFreezeLinuxMint()),
                ),
                if (isCurrentAbShared)
                  row(
                      label: Text(
                        translate('Password'),
                        style: style,
                      ),
                      input: Obx(
                        () => TextField(
                          controller: passwordController,
                          obscureText: !passwordVisible,
                          decoration: InputDecoration(
                            labelText: stateGlobal.isPortrait.isFalse
                                ? null
                                : translate('Password'),
                            suffixIcon: IconButton(
                              icon: Icon(
                                  passwordVisible
                                      ? Icons.visibility
                                      : Icons.visibility_off,
                                  color: MyTheme.lightTheme.primaryColor),
                              onPressed: () {
                                setState(() {
                                  passwordVisible = !passwordVisible;
                                });
                              },
                            ),
                          ),
                        ).workaroundFreezeLinuxMint(),
                      )),
                row(
                    label: Text(
                      translate('Note'),
                      style: style,
                    ),
                    input: Obx(
                      () => TextField(
                        controller: noteController,
                        maxLines: 3,
                        minLines: 1,
                        maxLength: 300,
                        decoration: InputDecoration(
                          labelText: stateGlobal.isPortrait.isFalse
                              ? null
                              : translate('Note'),
                        ),
                      ).workaroundFreezeLinuxMint(),
                    )),
                if (gFFI.abModel.currentAbTags.isNotEmpty)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      translate('Tags'),
                      style: style,
                    ),
                  ).marginOnly(top: 8, bottom: marginBottom),
                if (gFFI.abModel.currentAbTags.isNotEmpty)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: Wrap(
                      children: tags
                          .map((e) => AddressBookTag(
                              name: e,
                              tags: selectedTag,
                              onTap: () {
                                if (selectedTag.contains(e)) {
                                  selectedTag.remove(e);
                                } else {
                                  selectedTag.add(e);
                                }
                              },
                              showActionMenu: false))
                          .toList(growable: false),
                    ),
                  ),
              ],
            ),
            const SizedBox(
              height: 4.0,
            ),
            if (!gFFI.abModel.current.isPersonal())
              Row(children: [
                Icon(Icons.info, color: Colors.amber).marginOnly(right: 4),
                Text(
                  translate('share_warning_tip'),
                  style: TextStyle(fontSize: 12),
                )
              ]).marginSymmetric(vertical: 10),
            // NOT use Offstage to wrap LinearProgressIndicator
            if (isInProgress) const LinearProgressIndicator(),
          ],
        ),
        actions: [
          dialogButton("Cancel", onPressed: close, isOutline: true),
          dialogButton("OK", onPressed: submit),
        ],
        onSubmit: submit,
        onCancel: close,
      );
    });
  }
}
