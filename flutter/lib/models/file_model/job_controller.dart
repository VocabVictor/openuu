part of 'file_model.dart';

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

  void clear() {
    jobTable.clear();
    _transferConflictJobToBatch.clear();
    _transferConflictRememberBatchId = null;
    _transferConflictRememberOverrideConfirm = null;
    jobResultListener.clear();
  }
}

const _kOneWayFileTransferError = 'one-way-file-transfer-tip';
