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
part 'job_controller_jobs.dart';
part 'job_controller.dart';
part 'file_controller_transfer.dart';
part 'file_controller_actions.dart';

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
  bool _removeCheckboxRemember = false;
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

}
