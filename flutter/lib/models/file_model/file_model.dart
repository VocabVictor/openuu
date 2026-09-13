import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/utils/event_loop.dart';
import 'package:get/get.dart';
import 'package:path/path.dart' as path;
import 'package:flutter_hbb/native/unsupported_web.dart';

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
part 'file_controller.dart';
part 'file_model_dialogs.dart';

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

  bool fileConfirmCheckboxRemember = false;

}
