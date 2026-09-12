part of 'file_model.dart';

extension FileModelDialogs on FileModel {
  Future<void> postOverrideFileConfirm(Map<String, dynamic> evt) async {
    final id = int.tryParse(evt['id']?.toString() ?? '');
    if (id == null || !jobController.hasTransferConflictJob(id)) {
      debugPrint("Ignore stale override confirm event: $evt");
      return;
    }
    evtLoop.pushEvent(
        _FileDialogEvent(WeakReference(this), FileDialogType.overwrite, evt));
  }

  Future<void> overrideFileConfirm(Map<String, dynamic> evt,
      {bool? overrideConfirm, bool skip = false}) async {
    final id = int.tryParse(evt['id']?.toString() ?? '') ?? 0;
    if (id == 0 || !jobController.hasTransferConflictJob(id)) {
      debugPrint("Ignore override confirm for inactive job: $evt");
      return;
    }
    // If `skip == true`, it means to skip this file without showing dialog.
    // Because `resp` may be null after the user operation or the last remembered operation,
    // and we should distinguish them.
    final resp = overrideConfirm ??
        (!skip
            ? await showFileConfirmDialog(translate("Overwrite"),
                "${evt['read_path']}", true, evt['is_identical'] == "true")
            : null);
    if (!jobController.hasTransferConflictJob(id)) {
      debugPrint("Ignore override confirm result for inactive job: $evt");
      return;
    }
    if (false == resp) {
      await jobController.cancelTransferConflictBatch(id);
    } else {
      var need_override = false;
      if (resp == null) {
        // skip
        need_override = false;
      } else {
        // overwrite
        need_override = true;
      }
      // Update the loop config.
      if (fileConfirmCheckboxRemember) {
        jobController.rememberTransferConflictBatch(id, resp);
        evtLoop.setSkip(!need_override);
      }
      await bind.sessionSetConfirmOverrideFile(
          sessionId: sessionId,
          actId: id,
          fileNum: int.parse(evt['file_num']),
          needOverride: need_override,
          remember: fileConfirmCheckboxRemember,
          isUpload: evt['is_upload'] == "true");
    }
    // Update the loop config.
    if (fileConfirmCheckboxRemember) {
      evtLoop.setOverrideConfirm(resp);
    }
  }
}

extension FileModelSelection on FileModel {
  Future<bool?> showFileConfirmDialog(
      String title, String content, bool showCheckbox, bool isIdentical) async {
    fileConfirmCheckboxRemember = false;
    return await parent.target?.dialogManager.show<bool?>(
        (setState, Function(bool? v) close, context) {
      cancel() => close(false);
      submit() => close(true);
      return CustomAlertDialog(
        title: Row(
          children: [
            const Icon(Icons.warning_rounded, color: Colors.red),
            Text(title).paddingOnly(
              left: 10,
            ),
          ],
        ),
        contentBoxConstraints:
            BoxConstraints(minHeight: 100, minWidth: 400, maxWidth: 400),
        content: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(translate("This file exists, skip or overwrite this file?"),
                  style: const TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 5),
              Text(content),
              Offstage(
                offstage: !isIdentical,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const SizedBox(height: 12),
                    Text(translate("identical_file_tip"),
                        style: const TextStyle(fontWeight: FontWeight.w500))
                  ],
                ),
              ),
              showCheckbox
                  ? CheckboxListTile(
                      contentPadding: const EdgeInsets.all(0),
                      dense: true,
                      controlAffinity: ListTileControlAffinity.leading,
                      title: Text(
                        translate("Do this for all conflicts"),
                      ),
                      value: fileConfirmCheckboxRemember,
                      onChanged: (v) {
                        if (v == null) return;
                        setState(() => fileConfirmCheckboxRemember = v);
                      },
                    )
                  : const SizedBox.shrink()
            ]),
        actions: [
          dialogButton(
            "Cancel",
            icon: Icon(Icons.close_rounded),
            onPressed: cancel,
            isOutline: true,
          ),
          dialogButton(
            "Skip",
            icon: Icon(Icons.navigate_next_rounded),
            onPressed: () => close(null),
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

  void onSelectedFiles(dynamic obj) {
    localController.selectedItems.clear();

    try {
      int handleIndex = int.parse(obj['handleIndex']);
      final file = jsonDecode(obj['file']);
      var entry = Entry.fromJson(file);
      entry.path = entry.name;
      final otherSideData = remoteController.directoryData();
      final toPath = otherSideData.directory.path;
      final isWindows = otherSideData.options.isWindows;
      final showHidden = otherSideData.options.showHidden;
      final jobID = jobController.addTransferJob(entry, false);
      jobController.registerTransferConflictBatch([jobID],
          batchId: int.tryParse(obj['batchId']?.toString() ?? ''));
      webSendLocalFiles(
        handleIndex: handleIndex,
        actId: jobID,
        path: entry.path,
        to: PathUtil.join(toPath, entry.name, isWindows),
        fileNum: 0,
        includeHidden: showHidden,
        isRemote: false,
      );
    } catch (e) {
      debugPrint("Failed to decode onSelectedFiles: $e");
    }
  }

  void sendEmptyDirs(dynamic obj) {
    late final List<dynamic> emptyDirs;
    try {
      emptyDirs = jsonDecode(obj['dirs'] as String);
    } catch (e) {
      debugPrint("Failed to decode sendEmptyDirs: $e");
    }
    final otherSideData = remoteController.directoryData();
    final toPath = otherSideData.directory.path;
    final isPeerWindows = otherSideData.options.isWindows;

    final isLocalWindows = isWindows || isWebOnWindows;
    for (var dir in emptyDirs) {
      if (isLocalWindows != isPeerWindows) {
        dir = PathUtil.convert(dir, isLocalWindows, isPeerWindows);
      }
      var peerPath = PathUtil.join(toPath, dir, isPeerWindows);
      remoteController.createDirWithRemote(peerPath, true);
    }
  }
}
