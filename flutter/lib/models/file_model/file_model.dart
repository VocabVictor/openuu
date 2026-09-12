import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/utils/event_loop.dart';
import 'package:get/get.dart';
import 'package:path/path.dart' as path;
import 'package:flutter_hbb/web/dummy.dart'
    if (dart.library.html) 'package:flutter_hbb/web/web_unique.dart';

import '../../consts.dart';
import '../model.dart';
import '../platform_model.dart';
part 'entries.dart';
part 'jobs.dart';
part 'file_dialog.dart';
part 'fetcher.dart';

typedef GetSessionID = SessionID Function();
typedef GetDialogManager = OverlayDialogManager? Function();
typedef ReadRemoteDirectory = Future<void> Function(
    SessionID sessionId, String path, bool includeHidden);

class FileModel {
  final WeakReference<FFI> parent;
  // late final String sessionId;
  late final FileFetcher fileFetcher;
  late final JobController jobController;

  late final FileController localController;
  late final FileController remoteController;

  late final GetSessionID getSessionID;
  late final GetDialogManager getDialogManager;
  SessionID get sessionId => getSessionID();
  late final FileDialogEventLoop evtLoop;

  FileModel(this.parent) {
    getSessionID = () => parent.target!.sessionId;
    getDialogManager = () => parent.target?.dialogManager;
    fileFetcher = FileFetcher(getSessionID);
    jobController = JobController(getSessionID, getDialogManager);
    localController = FileController(
        isLocal: true,
        getSessionID: getSessionID,
        rootState: parent,
        jobController: jobController,
        fileFetcher: fileFetcher,
        getOtherSideDirectoryData: () => remoteController.directoryData());
    remoteController = FileController(
        isLocal: false,
        getSessionID: getSessionID,
        rootState: parent,
        jobController: jobController,
        fileFetcher: fileFetcher,
        getOtherSideDirectoryData: () => localController.directoryData());
    evtLoop = FileDialogEventLoop();
  }

  Future<void> onReady() async {
    fileFetcher.beginRemoteSession();
    await evtLoop.onReady();
    if (!isWeb) await localController.onReady();
    await remoteController.onReady();
  }

  Future<void> close() async {
    await evtLoop.close();
    parent.target?.dialogManager.dismissAll();
    await localController.close();
    await remoteController.close();
  }

  Future<void> refreshAll() async {
    if (!isWeb) await localController.refresh();
    await remoteController.refresh();
  }

  void receiveFileDir(Map<String, dynamic> evt) {
    if (evt['is_local'] == "false") {
      // init remote home, the remote connection will send one dir event when established. TODO opt
      remoteController.initDirAndHome(evt);
    }
    fileFetcher.tryCompleteTask(evt['value'], evt['is_local']);
  }

  void receiveEmptyDirs(Map<String, dynamic> evt) {
    fileFetcher.tryCompleteEmptyDirsTask(evt['value'], evt['is_local']);
  }

  // This method fixes a deadlock that occurred when the previous code directly
  // called jobController.jobError(evt) in the job_error event handler.
  //
  // The problem with directly calling jobController.jobError():
  //   1. fetchDirectoryRecursiveToRemove(jobID) registers readRecursiveTasks[jobID]
  //      and waits for completion
  //   2. If the remote has no permission (or some other errors), it returns a FileTransferError
  //   3. The error triggers job_error event, which called jobController.jobError()
  //   4. jobController.jobError() calls getJob(jobID) to find the job in jobTable
  //   5. But addDeleteDirJob() is called AFTER fetchDirectoryRecursiveToRemove(),
  //      so the job doesn't exist yet in jobTable
  //   6. Result: jobController.jobError() does nothing useful, and
  //      readRecursiveTasks[jobID] never completes, causing a 2s timeout
  //
  // Solution: Before calling jobController.jobError(), we first check if there's
  // a pending readRecursiveTasks with this ID and complete it with the error.
  void handleJobError(Map<String, dynamic> evt) {
    final id = int.tryParse(evt['id']?.toString() ?? '');
    if (id != null) {
      final err = evt['err']?.toString() ?? 'Unknown error';
      if (id == 0) {
        fileFetcher.tryCompleteRemoteTaskWithError(err);
      } else {
        fileFetcher.tryCompleteRecursiveTaskWithError(id, err);
      }
    }
    // Always call jobController.jobError(evt) to ensure all error events are processed,
    // even if the event does not have a valid job ID. This allows for generic error handling
    // or logging of unexpected errors.
    jobController.jobError(evt);
  }

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

  bool fileConfirmCheckboxRemember = false;

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

class DirectoryData {
  final DirectoryOptions options;
  final FileDirectory directory;
  DirectoryData(this.directory, this.options);
}

class FileController {
  final bool isLocal;
  final GetSessionID getSessionID;
  SessionID get sessionId => getSessionID();

  final FileFetcher fileFetcher;

  final options = DirectoryOptions().obs;
  final directory = FileDirectory().obs;

  final history = RxList<String>.empty(growable: true);
  final sortBy = SortBy.name.obs;
  var sortAscending = true;
  // Incremented for each navigation; only the latest generation applies results.
  int _directoryRequestGeneration = 0;
  final JobController jobController;
  final WeakReference<FFI> rootState;

  final DirectoryData Function() getOtherSideDirectoryData;
  late final SelectedItems selectedItems = SelectedItems(isLocal: isLocal);

  FileController(
      {required this.isLocal,
      required this.getSessionID,
      required this.rootState,
      required this.jobController,
      required this.fileFetcher,
      required this.getOtherSideDirectoryData});

  String get homePath => options.value.home;
  void set homePath(String path) => options.value.home = path;
  OverlayDialogManager? get dialogManager => rootState.target?.dialogManager;

  bool _isPathAllowed(String candidate) {
    if (!isAndroid || !isLocal) return true;
    if (homePath.isEmpty || candidate.isEmpty) return false;
    final home = PathUtil.posixContext.normalize(homePath);
    final target = PathUtil.posixContext.normalize(candidate);
    return target == home || PathUtil.posixContext.isWithin(home, target);
  }

  String get shortPath {
    final dirPath = directory.value.path;
    if (dirPath.startsWith(homePath)) {
      var path = dirPath.replaceFirst(homePath, "");
      if (path.isEmpty) return "";
      if (path[0] == "/" || path[0] == "\\") {
        // remove more '/' or '\'
        path = path.replaceFirst(path[0], "");
      }
      return path;
    } else {
      return dirPath.replaceFirst(homePath, "");
    }
  }

  DirectoryData directoryData() {
    return DirectoryData(directory.value, options.value);
  }

  Future<void> onReady() async {
    if (isLocal) {
      options.value.home = await bind.mainGetHomeDir();
    }
    options.value.showHidden = (await bind.sessionGetPeerOption(
            sessionId: sessionId,
            name: isLocal ? "local_show_hidden" : "remote_show_hidden"))
        .isNotEmpty;
    options.value.isWindows = isLocal
        ? isWindows
        : rootState.target?.ffiModel.pi.platform == kPeerPlatformWindows;

    await Future.delayed(Duration(milliseconds: 100));

    var savedDir = (await bind.sessionGetPeerOption(
        sessionId: sessionId, name: isLocal ? "local_dir" : "remote_dir"));
    if (savedDir.isNotEmpty && !_isPathAllowed(savedDir)) {
      savedDir = options.value.home;
      await bind.sessionPeerOption(
        sessionId: sessionId, name: "local_dir", value: savedDir);
    }
    Future<bool> tryOpenReadyDirs() async {
      final dirs = <String>{
        if (directory.value.path.isNotEmpty) directory.value.path,
        if (savedDir.isNotEmpty) savedDir,
        options.value.home,
      };
      for (final dir in dirs) {
        if (await _openDirectoryPath(dir, isBack: true)) {
          return true;
        }
      }
      return false;
    }

    var opened = await tryOpenReadyDirs();

    await Future.delayed(Duration(seconds: 1));

    if (!opened) {
      // The peer may become ready during the reconnect delay, so retry the
      // same candidates instead of only retrying the default home directory.
      await tryOpenReadyDirs();
    }
  }

  Future<void> close() async {
    // save config
    Map<String, String> msgMap = {};
    msgMap[isLocal ? "local_dir" : "remote_dir"] = directory.value.path;
    msgMap[isLocal ? "local_show_hidden" : "remote_show_hidden"] =
        options.value.showHidden ? "Y" : "";
    for (final msg in msgMap.entries) {
      await bind.sessionPeerOption(
          sessionId: sessionId, name: msg.key, value: msg.value);
    }
    directory.value.clear();
    options.value.clear();
  }

  void toggleShowHidden({bool? showHidden}) {
    options.value.showHidden = showHidden ?? !options.value.showHidden;
    refresh();
  }

  void changeSortStyle(SortBy sort, {bool? isLocal, bool ascending = true}) {
    sortBy.value = sort;
    sortAscending = ascending;
    directory.update((dir) {
      dir?.changeSortStyle(sort, ascending: ascending);
    });
  }

  Future<bool> refresh() async {
    // "." can be both a refresh command and a real remote directory path.
    // Refresh must bypass openDirectory's command dispatch to avoid recursion.
    return await _openDirectoryPath(directory.value.path, isBack: true);
  }

  Future<bool> openDirectory(String path, {bool isBack = false}) async {
    if (!isBack && path == ".") {
      return await refresh();
    }
    if (!isBack && path == "..") {
      return await _goToParentDirectory(isBack: isBack);
    }
    return await _openDirectoryPath(path, isBack: isBack);
  }

  Future<bool> _openDirectoryPath(String path, {bool isBack = false}) async {
    if (!_isPathAllowed(path)) {
      return false;
    }
    if (!isBack) {
      pushHistory();
    }
    final showHidden = options.value.showHidden;
    final isWindows = options.value.isWindows;
    // process /C:\ -> C:\ on Windows
    if (isWindows && path.length > 1 && path[0] == '/') {
      path = path.substring(1);
      if (path[path.length - 1] != '\\') {
        path = "$path\\";
      }
    }
    final requestGeneration = ++_directoryRequestGeneration;
    try {
      final fd = await fileFetcher.fetchDirectory(path, isLocal, showHidden);
      if (requestGeneration != _directoryRequestGeneration) {
        return true;
      }
      fd.format(isWindows, sort: sortBy.value);
      selectedItems.reconcile(fd.entries);
      directory.value = fd;
      return true;
    } catch (e) {
      if (requestGeneration != _directoryRequestGeneration) {
        return true;
      }
      debugPrint("Failed to openDirectory $path: $e");
      return false;
    }
  }

  void pushHistory() {
    if (history.isNotEmpty && history.last == directory.value.path) {
      return;
    }
    history.add(directory.value.path);
  }

  void goToHomeDirectory() {
    if (isLocal) {
      openDirectory(homePath);
      return;
    }
    homePath = "";
    openDirectory(homePath);
  }

  void goBack() {
    if (history.isEmpty) return;
    final path = history.removeAt(history.length - 1);
    if (path.isEmpty) return;
    if (directory.value.path == path) {
      goBack();
      return;
    }
    unawaited(_openDirectoryPath(path, isBack: true).then<void>((_) {}));
  }

  void goToParentDirectory() {
    unawaited(_goToParentDirectory().then<void>((_) {}));
  }

  Future<bool> _goToParentDirectory({bool isBack = false}) async {
    final isWindows = options.value.isWindows;
    final dirPath = directory.value.path;
    var parent = PathUtil.dirname(dirPath, isWindows);
    if (!_isPathAllowed(parent)) {
      return true;
    }
    // specially for C:\, D:\, goto '/'
    if (parent == dirPath && isWindows) {
      return await _openDirectoryPath('/', isBack: isBack);
    }
    return await _openDirectoryPath(parent, isBack: isBack);
  }

  // TODO deprecated this
  void initDirAndHome(Map<String, dynamic> evt) {
    try {
      final fd = FileDirectory.fromJson(jsonDecode(evt['value']));
      final isHomeResponse = fileFetcher.isLikelyRemoteHomeResponse(fd.path);
      fd.format(options.value.isWindows, sort: sortBy.value);
      if (fd.id > 0) {
        final jobIndex = jobController.getJob(fd.id);
        if (jobIndex != -1) {
          final job = jobController.jobTable[jobIndex];
          var totalSize = 0;
          var fileCount = fd.entries.length;
          for (var element in fd.entries) {
            totalSize += element.size;
          }
          job.totalSize = totalSize;
          job.fileCount = fileCount;
          debugPrint("update receive details: ${fd.path}");
          jobController.jobTable.refresh();
        }
      } else if (options.value.home.isEmpty && isHomeResponse) {
        options.value.home = fd.path;
        debugPrint("init remote home: ${fd.path}");
        if (_directoryRequestGeneration == 0) {
          directory.value = fd;
        }
      }
    } catch (e) {
      debugPrint("initDirAndHome err=$e");
    }
  }

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

  bool _removeCheckboxRemember = false;

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

const _kOneWayFileTransferError = 'one-way-file-transfer-tip';

class JobController {
  static final JobID jobID = JobID();
  final jobTable = List<JobProgress>.empty(growable: true).obs;
  final jobResultListener = JobResultListener<Map<String, dynamic>>();
  int _nextTransferConflictBatchId = 1;
  final Map<int, int> _transferConflictJobToBatch = {};
  int? _transferConflictRememberBatchId;
  bool? _transferConflictRememberOverrideConfirm;
  final GetSessionID getSessionID;
  final GetDialogManager getDialogManager;
  SessionID get sessionId => getSessionID();
  OverlayDialogManager? get alogManager => getDialogManager();
  int _lastTimeShowMsgbox = DateTime.now().millisecondsSinceEpoch;

  JobController(this.getSessionID, this.getDialogManager);

  int getJob(int id) {
    return jobTable.indexWhere((element) => element.id == id);
  }

  void registerTransferConflictBatch(Iterable<int> jobIds, {int? batchId}) {
    final ids = jobIds.toList(growable: false);
    if (ids.isEmpty) {
      return;
    }
    batchId ??= _nextTransferConflictBatchId++;
    if (batchId >= _nextTransferConflictBatchId) {
      _nextTransferConflictBatchId = batchId + 1;
    }
    for (final jobId in ids) {
      _transferConflictJobToBatch[jobId] = batchId;
    }
  }

  int? transferConflictBatchId(int jobId) {
    return _transferConflictJobToBatch[jobId];
  }

  bool hasTransferConflictJob(int jobId) {
    return transferConflictBatchId(jobId) != null;
  }

  bool isTransferConflictRememberBatch(int? batchId) {
    return batchId != null && batchId == _transferConflictRememberBatchId;
  }

  bool? transferConflictRememberOverrideConfirm(int? batchId) {
    if (!isTransferConflictRememberBatch(batchId)) {
      return null;
    }
    return _transferConflictRememberOverrideConfirm;
  }

  void rememberTransferConflictBatch(int jobId, bool? overrideConfirm) {
    _transferConflictRememberBatchId = _transferConflictJobToBatch[jobId];
    _transferConflictRememberOverrideConfirm = overrideConfirm;
  }

  void unregisterTransferConflictJob(int jobId) {
    final batchId = _transferConflictJobToBatch.remove(jobId);
    if (batchId == null) {
      return;
    }
    if (!_transferConflictJobToBatch.containsValue(batchId)) {
      if (_transferConflictRememberBatchId == batchId) {
        _transferConflictRememberBatchId = null;
        _transferConflictRememberOverrideConfirm = null;
      }
    }
  }

  // return jobID
  int addTransferJob(Entry from, bool isRemoteToLocal) {
    final jobID = JobController.jobID.next();
    jobTable.add(JobProgress()
      ..type = JobType.transfer
      ..fileName = path.basename(from.path)
      ..jobName = from.path
      ..totalSize = from.size
      ..state = JobState.inProgress
      ..id = jobID
      ..isRemoteToLocal = isRemoteToLocal);
    return jobID;
  }

  int addDeleteFileJob(Entry file, bool isRemote) {
    final jobID = JobController.jobID.next();
    jobTable.add(JobProgress()
      ..type = JobType.deleteFile
      ..fileName = path.basename(file.path)
      ..jobName = file.path
      ..totalSize = file.size
      ..state = JobState.none
      ..id = jobID
      ..isRemoteToLocal = isRemote);
    return jobID;
  }

  int addDeleteDirJob(Entry file, bool isRemote, int fileCount) {
    final jobID = JobController.jobID.next();
    jobTable.add(JobProgress()
      ..type = JobType.deleteDir
      ..fileName = path.basename(file.path)
      ..jobName = file.path
      ..fileCount = fileCount
      ..totalSize = file.size
      ..state = JobState.none
      ..id = jobID
      ..isRemoteToLocal = isRemote);
    return jobID;
  }

  void tryUpdateJobProgress(Map<String, dynamic> evt) {
    try {
      int id = int.parse(evt['id']);
      // id = index + 1
      final jobIndex = getJob(id);
      if (jobIndex >= 0 && jobTable.length > jobIndex) {
        final job = jobTable[jobIndex];
        job.fileNum = int.parse(evt['file_num']);
        job.speed = double.parse(evt['speed']);
        job.finishedSize = int.parse(evt['finished_size']);
        job.recvJobRes = true;
        jobTable.refresh();
      }
    } catch (e) {
      debugPrint("Failed to tryUpdateJobProgress, evt: ${evt.toString()}");
    }
  }

  Future<bool> jobDone(Map<String, dynamic> evt) async {
    if (jobResultListener.isListening) {
      jobResultListener.complete(evt);
      // return;
    }
    int id = -1;
    int? fileNum = 0;
    double? speed = 0;
    try {
      id = int.parse(evt['id']);
    } catch (_) {}
    final jobIndex = getJob(id);
    if (jobIndex == -1) {
      unregisterTransferConflictJob(id);
      return true;
    }
    final job = jobTable[jobIndex];
    job.recvJobRes = true;
    if (job.type == JobType.deleteFile) {
      job.state = JobState.done;
    } else if (job.type == JobType.deleteDir) {
      try {
        fileNum = int.tryParse(evt['file_num']);
      } catch (_) {}
      if (fileNum != null) {
        if (fileNum < job.fileNum) return true; // file_num can be 0 at last
        job.fileNum = fileNum;
        if (fileNum >= job.fileCount - 1) {
          job.state = JobState.done;
        }
      }
    } else {
      try {
        fileNum = int.tryParse(evt['file_num']);
        speed = double.tryParse(evt['speed']);
      } catch (_) {}
      if (fileNum != null) job.fileNum = fileNum;
      if (speed != null) job.speed = speed;
      job.state = JobState.done;
    }
    jobTable.refresh();
    if (job.state == JobState.done || job.state == JobState.error) {
      unregisterTransferConflictJob(id);
    }
    if (job.type == JobType.deleteDir) {
      return job.state == JobState.done;
    } else {
      return true;
    }
  }

  void jobError(Map<String, dynamic> evt) {
    final err = evt['err'].toString();
    final id = int.tryParse(evt['id']?.toString() ?? '');
    if (id == null) {
      debugPrint("Ignore job error with invalid id: $evt");
      return;
    }
    int jobIndex = getJob(id);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      if (job.state == JobState.done && job.err == "cancel") return;
      job.state = JobState.error;
      job.err = err;
      job.recvJobRes = true;
      if (job.type == JobType.transfer) {
        int? fileNum = int.tryParse(evt['file_num']);
        if (fileNum != null) job.fileNum = fileNum;
        if (err == "skipped") {
          job.state = JobState.done;
          job.finishedSize = job.totalSize;
        }
      } else if (job.type == JobType.deleteDir) {
        if (jobResultListener.isListening) {
          jobResultListener.complete(evt);
        }
        int? fileNum = int.tryParse(evt['file_num']);
        if (fileNum != null) job.fileNum = fileNum;
      } else if (job.type == JobType.deleteFile) {
        if (jobResultListener.isListening) {
          jobResultListener.complete(evt);
        }
      }
      jobTable.refresh();
      if (job.state == JobState.done || job.state == JobState.error) {
        unregisterTransferConflictJob(job.id);
      }
    } else {
      unregisterTransferConflictJob(id);
    }
    if (err == _kOneWayFileTransferError) {
      if (DateTime.now().millisecondsSinceEpoch - _lastTimeShowMsgbox > 3000) {
        final dm = alogManager;
        if (dm != null) {
          _lastTimeShowMsgbox = DateTime.now().millisecondsSinceEpoch;
          msgBox(sessionId, 'custom-nocancel', 'Error', err, '', dm);
        }
      }
    }
    debugPrint("jobError $evt");
  }

  void updateJobStatus(int id,
      {int? file_num, String? error, JobState? state}) {
    final jobIndex = getJob(id);
    if (jobIndex < 0) return;
    final job = jobTable[jobIndex];
    job.recvJobRes = true;
    if (file_num != null) {
      job.fileNum = file_num;
    }
    if (error != null) {
      job.err = error;
      job.state = JobState.error;
    }
    if (state != null) {
      job.state = state;
    }
    if (job.type == JobType.deleteFile && error == null) {
      job.state = JobState.done;
    }
    jobTable.refresh();
  }

  Future<void> cancelJob(int id) async {
    unregisterTransferConflictJob(id);
    await bind.sessionCancelJob(sessionId: sessionId, actId: id);
  }

  Future<void> cancelTransferConflictBatch(int jobId) async {
    final batchId = _transferConflictJobToBatch[jobId];
    final batchJobIds = batchId == null ? [jobId] : <int>[];
    if (batchId != null) {
      for (final entry in _transferConflictJobToBatch.entries) {
        if (entry.value == batchId) {
          batchJobIds.add(entry.key);
        }
      }
      for (final id in batchJobIds) {
        unregisterTransferConflictJob(id);
      }
    }
    final jobIdsToCancel = batchJobIds.toSet();
    for (final job in jobTable) {
      if (!jobIdsToCancel.contains(job.id) || job.state == JobState.done) {
        continue;
      }
      job.state = JobState.done;
      job.err = "cancel";
      job.recvJobRes = true;
    }
    jobTable.refresh();
    for (final id in batchJobIds) {
      try {
        await bind.sessionCancelJob(sessionId: sessionId, actId: id);
      } catch (e) {
        debugPrint("Failed to cancel transfer job $id in conflict batch: $e");
      }
    }
  }

  Future<void> loadLastJob(Map<String, dynamic> evt) async {
    debugPrint("load last job: $evt");
    Map<String, dynamic> jobDetail = json.decode(evt['value']);
    String remote = jobDetail['remote'];
    String to = jobDetail['to'];
    bool showHidden = jobDetail['show_hidden'];
    int fileNum = jobDetail['file_num'];
    bool isRemote = jobDetail['is_remote'];
    bool isAutoStart = jobDetail['auto_start'] == true;
    int currJobId = -1;
    if (isAutoStart) {
      // Ensure jobDetail['id'] exists and is an int
      if (jobDetail.containsKey('id') &&
          jobDetail['id'] != null &&
          jobDetail['id'] is int) {
        currJobId = jobDetail['id'];
      }
    }
    if (currJobId < 0) {
      // If id is missing or invalid, disable auto-start and assign a new job id
      isAutoStart = false;
      currJobId = JobController.jobID.next();
    }

    if (!isAutoStart) {
      if (!(isDesktop || isWebDesktop)) {
        // Don't add to job table if not auto start on mobile.
        // Because mobile does not support job list view now.
        return;
      }

      // Add to job table if not auto start on desktop.
      String fileName = path.basename(isRemote ? remote : to);
      final jobProgress = JobProgress()
        ..type = JobType.transfer
        ..fileName = fileName
        ..jobName = isRemote ? remote : to
        ..id = currJobId
        ..isRemoteToLocal = isRemote
        ..fileNum = fileNum
        ..remote = remote
        ..to = to
        ..showHidden = showHidden
        ..state = JobState.paused;
      jobTable.add(jobProgress);
    }
    registerTransferConflictBatch([currJobId]);
    await bind.sessionAddJob(
      sessionId: sessionId,
      isRemote: isRemote,
      includeHidden: showHidden,
      actId: currJobId,
      path: isRemote ? remote : to,
      to: isRemote ? to : remote,
      fileNum: fileNum,
    );

    if (isAutoStart) {
      await bind.sessionResumeJob(
          sessionId: sessionId, actId: currJobId, isRemote: isRemote);
    }
  }

  Future<void> pauseJob(int jobId) async {
    final index = getJob(jobId);
    if (index < 0) return;
    final job = jobTable[index];
    if (job.type != JobType.transfer || job.state != JobState.inProgress) return;
    await bind.sessionPeerOption(sessionId: sessionId, name: 'file-transfer-pause',
      value: jsonEncode([jobId, true]));
    if (job.state != JobState.inProgress) return;
    job.livePaused = true;
    job.state = JobState.paused;
    job.speed = 0;
    jobTable.refresh();
  }

  void resumeJob(int jobId) {
    final jobIndex = getJob(jobId);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      if (job.livePaused) {
        bind.sessionPeerOption(sessionId: sessionId, name: 'file-transfer-pause',
          value: jsonEncode([jobId, false]));
        job.livePaused = false;
        job.state = JobState.inProgress;
        jobTable.refresh();
        return;
      }
      bind.sessionResumeJob(
          sessionId: sessionId, actId: job.id, isRemote: job.isRemoteToLocal);
      job.state = JobState.inProgress;
      jobTable.refresh();
    } else {
      debugPrint("jobId $jobId is not exists");
    }
  }

  void updateFolderFiles(Map<String, dynamic> evt) {
    // ret: "{\"id\":1,\"num_entries\":12,\"total_size\":1264822.0}"
    Map<String, dynamic> info = json.decode(evt['info']);
    int id = info['id'];
    int num_entries = info['num_entries'];
    double total_size = info['total_size'];
    final jobIndex = getJob(id);
    if (jobIndex != -1) {
      final job = jobTable[jobIndex];
      job.fileCount = num_entries;
      job.totalSize = total_size.toInt();
      jobTable.refresh();
    }
    debugPrint("update folder files: $info");
  }

  void clear() {
    jobTable.clear();
    _transferConflictJobToBatch.clear();
    _transferConflictRememberBatchId = null;
    _transferConflictRememberOverrideConfirm = null;
    jobResultListener.clear();
  }
}
