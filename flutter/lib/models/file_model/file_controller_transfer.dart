part of 'file_model.dart';

extension FileControllerTransfer on FileController {
  /// sendFiles from current side (FileController.isLocal) to other side (SelectedItems).
  Future<void> sendFiles(
      SelectedItems items, DirectoryData otherSideData) async {
    /// ignore wrong items side status
    if (items.isLocal != isLocal) {
      return;
    }

    // alias
    final isRemoteToLocal = !isLocal;

    final toPath = otherSideData.directory.path;
    final isWindows = otherSideData.options.isWindows;
    final showHidden = otherSideData.options.showHidden;
    final transferJobs = <(Entry, int)>[];
    final transferJobIds = <int>[];
    for (var from in items.items) {
      final jobID = jobController.addTransferJob(from, isRemoteToLocal);
      transferJobs.add((from, jobID));
      transferJobIds.add(jobID);
    }
    jobController.registerTransferConflictBatch(transferJobIds);
    for (final (from, jobID) in transferJobs) {
      bind.sessionSendFiles(
          sessionId: sessionId,
          actId: jobID,
          path: from.path,
          to: PathUtil.join(toPath, from.name, isWindows),
          fileNum: 0,
          includeHidden: showHidden,
          isRemote: isRemoteToLocal,
          isDir: from.isDirectory);
      debugPrint(
          "path: ${from.path}, toPath: $toPath, to: ${PathUtil.join(toPath, from.name, isWindows)}");
    }

    if (isWeb ||
        (!isLocal &&
            versionCmp(rootState.target!.ffiModel.pi.version, '1.3.3') < 0)) {
      return;
    }

    final List<Entry> entrys = items.items.toList();
    var isRemote = isLocal == true ? true : false;

    await Future.forEach(entrys, (Entry item) async {
      if (!item.isDirectory) {
        return;
      }

      final List<String> paths = [];

      final emptyDirs =
          await fileFetcher.readEmptyDirs(item.path, isLocal, showHidden);

      if (emptyDirs.isEmpty) {
        return;
      } else {
        for (var dir in emptyDirs) {
          paths.add(dir.path);
        }
      }

      final dirs = paths.map((path) {
        return PathUtil.getOtherSidePath(directory.value.path, path,
            options.value.isWindows, toPath, isWindows);
      });

      for (var dir in dirs) {
        createDirWithRemote(dir, isRemote);
      }
    });
  }

  Future<void> removeAction(SelectedItems items) async {
    _removeCheckboxRemember = false;
    if (items.isLocal != isLocal) {
      debugPrint("Failed to removeFile, wrong files");
      return;
    }
    final isWindows = options.value.isWindows;
    await Future.forEach(items.items, (Entry item) async {
      final jobID = JobController.jobID.next();
      var title = "";
      var content = "";
      late final List<Entry> entries;
      if (item.isFile) {
        title = translate("Are you sure you want to delete this file?");
        content = item.name;
        entries = [item];
      } else if (item.isDirectory) {
        title = translate("Not an empty directory");
        dialogManager?.showLoading(translate("Waiting"));
        final FileDirectory fd;
        try {
          fd = await fileFetcher.fetchDirectoryRecursiveToRemove(
              jobID, item.path, items.isLocal, true);
        } catch (e) {
          dialogManager?.dismissAll();
          final dm = dialogManager;
          if (dm != null) {
            msgBox(sessionId, 'custom-error-nook-nocancel-hasclose',
                translate("Error"), e.toString(), '', dm);
          } else {
            debugPrint("removeAction error msgbox failed: $e");
          }
          return;
        }
        if (fd.path.isEmpty) {
          fd.path = item.path;
        }
        fd.format(isWindows);
        dialogManager?.dismissAll();
        if (fd.entries.isEmpty) {
          var deleteJobId = jobController.addDeleteDirJob(item, !isLocal, 0);
          final confirm = await showRemoveDialog(
              translate(
                  "Are you sure you want to delete this empty directory?"),
              item.name,
              false);
          if (confirm == true) {
            await sendRemoveEmptyDir(
              item.path,
              0,
              deleteJobId,
            );
          } else {
            jobController.updateJobStatus(deleteJobId,
                error: "cancel", state: JobState.done);
          }
          return;
        }
        entries = fd.entries;
      } else {
        entries = [];
      }
      int deleteJobId;
      if (item.isDirectory) {
        deleteJobId =
            jobController.addDeleteDirJob(item, !isLocal, entries.length);
      } else {
        deleteJobId = jobController.addDeleteFileJob(item, !isLocal);
      }

      for (var i = 0; i < entries.length; i++) {
        final dirShow = item.isDirectory
            ? "${translate("Are you sure you want to delete the file of this directory?")}\n"
            : "";
        final count = entries.length > 1 ? "${i + 1}/${entries.length}" : "";
        content = "$dirShow\n\n${entries[i].path}".trim();
        final confirm = await showRemoveDialog(
          count.isEmpty ? title : "$title ($count)",
          content,
          item.isDirectory,
        );
        try {
          if (confirm == true) {
            sendRemoveFile(entries[i].path, i, deleteJobId);
            final res = await jobController.jobResultListener.start();
            // handle remove res;
            if (item.isDirectory &&
                res['file_num'] == (entries.length - 1).toString()) {
              await sendRemoveEmptyDir(item.path, i, deleteJobId);
            }
          } else {
            jobController.updateJobStatus(deleteJobId,
                file_num: i, error: "cancel");
          }
          if (_removeCheckboxRemember) {
            if (confirm == true) {
              for (var j = i + 1; j < entries.length; j++) {
                sendRemoveFile(entries[j].path, j, deleteJobId);
                final res = await jobController.jobResultListener.start();
                if (item.isDirectory &&
                    res['file_num'] == (entries.length - 1).toString()) {
                  await sendRemoveEmptyDir(item.path, i, deleteJobId);
                }
              }
            } else {
              jobController.updateJobStatus(deleteJobId,
                  error: "cancel",
                  file_num: entries.length,
                  state: JobState.done);
            }
            break;
          }
        } catch (e) {
          print("remove error: $e");
        }
      }
    });
    refresh();
  }
}
