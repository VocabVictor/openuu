part of 'file_model.dart';

extension FileControllerActions on FileController {
  Future<bool?> showRemoveDialog(
      String title, String content, bool showCheckbox) async {
    return await dialogManager?.show<bool>(
        (setState, Function(bool v) close, context) {
      cancel() => close(false);
      submit() => close(true);
      return CustomAlertDialog(
        title: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.warning_rounded, color: Colors.red),
            Expanded(
              child: Text(title).paddingOnly(
                left: 10,
              ),
            ),
          ],
        ),
        contentBoxConstraints:
            BoxConstraints(minHeight: 100, minWidth: 400, maxWidth: 400),
        content: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(content),
            Text(
              translate("This is irreversible!"),
              style: const TextStyle(
                fontWeight: FontWeight.bold,
                color: Colors.red,
              ),
            ).paddingOnly(top: 20),
            showCheckbox
                ? CheckboxListTile(
                    contentPadding: const EdgeInsets.all(0),
                    dense: true,
                    controlAffinity: ListTileControlAffinity.leading,
                    title: Text(
                      translate("Do this for all conflicts"),
                    ),
                    value: _removeCheckboxRemember,
                    onChanged: (v) {
                      if (v == null) return;
                      setState(() => _removeCheckboxRemember = v);
                    },
                  )
                : const SizedBox.shrink()
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
    }, useAnimation: false);
  }

  void sendRemoveFile(String path, int fileNum, int actId) {
    bind.sessionRemoveFile(
        sessionId: sessionId,
        actId: actId,
        path: path,
        isRemote: !isLocal,
        fileNum: fileNum);
  }

  Future<void> sendRemoveEmptyDir(String path, int fileNum, int actId) async {
    history.removeWhere((element) => element.contains(path));
    await bind.sessionRemoveAllEmptyDirs(
        sessionId: sessionId, actId: actId, path: path, isRemote: !isLocal);
  }

  Future<void> createDirWithRemote(String path, bool isRemote) async {
    bind.sessionCreateDir(
        sessionId: sessionId,
        actId: JobController.jobID.next(),
        path: path,
        isRemote: isRemote);
  }

  Future<void> createDir(String path) async {
    await createDirWithRemote(path, !isLocal);
  }

  Future<void> renameAction(Entry item, bool isLocal) async {
    final textEditingController = TextEditingController(text: item.name);
    String? errorText;
    dialogManager?.show((setState, close, context) {
      textEditingController.addListener(() {
        if (errorText != null) {
          setState(() {
            errorText = null;
          });
        }
      });
      submit() async {
        final newName = textEditingController.text;
        if (newName.isEmpty || newName == item.name) {
          close();
          return;
        }
        if (directory.value.entries.any((e) => e.name == newName)) {
          setState(() {
            errorText = translate("Already exists");
          });
          return;
        }
        if (!PathUtil.validName(newName, options.value.isWindows)) {
          setState(() {
            if (item.isDirectory) {
              errorText = translate("Invalid folder name");
            } else {
              errorText = translate("Invalid file name");
            }
          });
          return;
        }
        await bind.sessionRenameFile(
            sessionId: sessionId,
            actId: JobController.jobID.next(),
            path: item.path,
            newName: newName,
            isRemote: !isLocal);
        close();
      }

      return CustomAlertDialog(
        content: Column(
          children: [
            DialogTextField(
              title: '${translate('Rename')} ${item.name}',
              controller: textEditingController,
              errorText: errorText,
            ),
          ],
        ),
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
    });
  }
}
