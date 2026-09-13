part of 'file_model.dart';

extension JobControllerJobs on JobController {
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
      if (!isDesktop) {
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
}
