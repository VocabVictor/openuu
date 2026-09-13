part of 'file_model.dart';

enum SortBy {
  name,
  type,
  modified,
  size;

  @override
  String toString() {
    final str = this.name.toString();
    return "${str[0].toUpperCase()}${str.substring(1)}";
  }
}

class JobID {
  int _count = 0;
  int next() {
    try {
      String v = bind.mainGetCommonSync(key: 'transfer-job-id');
      return int.parse(v);
    } catch (e) {
      debugPrint("Failed to get transfer job id: $e");
    }
    // Fall back to a local counter if the id could not be read.
    _count++;
    return _count;
  }
}

enum JobState { none, inProgress, done, error, paused }

extension JobStateDisplay on JobState {
  String display() {
    switch (this) {
      case JobState.none:
        return translate("Waiting");
      case JobState.inProgress:
        return translate("Transfer file");
      case JobState.done:
        return translate("Finished");
      case JobState.error:
        return translate("Error");
      default:
        return "";
    }
  }
}

enum JobType { none, transfer, deleteFile, deleteDir }

class JobProgress {
  bool livePaused = false;
  JobType type = JobType.none;
  JobState state = JobState.none;
  var recvJobRes = false;
  var id = 0;
  var fileNum = 0;
  var speed = 0.0;
  var finishedSize = 0;
  var totalSize = 0;
  var fileCount = 0;
  // [isRemote == true] means [remote -> local]
  // var isRemote = false;
  // to-do use enum
  var isRemoteToLocal = false;
  var jobName = "";
  var fileName = "";
  var remote = "";
  var to = "";
  var showHidden = false;
  var err = "";
  int lastTransferredSize = 0;

  double get percent =>
      totalSize > 0 ? (finishedSize.toDouble() / totalSize) : 0.0;
  String get percentText => '${(percent * 100).toStringAsFixed(0)}%';

  clear() {
    type = JobType.none;
    state = JobState.none;
    recvJobRes = false;
    id = 0;
    fileNum = 0;
    speed = 0;
    finishedSize = 0;
    jobName = "";
    fileName = "";
    fileCount = 0;
    remote = "";
    to = "";
    err = "";
  }

  String display() {
    if (type == JobType.transfer) {
      if (state == JobState.done && err == "cancel") {
        return translate("Cancel");
      }
      if (state == JobState.done && err == "skipped") {
        return translate("Skipped");
      }
    } else if (type == JobType.deleteFile) {
      if (err == "cancel") {
        return translate("Cancel");
      }
    }

    return state.display();
  }

  String getStatus() {
    int handledFileCount = recvJobRes ? fileNum + 1 : fileNum;
    if (handledFileCount >= fileCount) {
      handledFileCount = fileCount;
    }
    if (state == JobState.done) {
      handledFileCount = fileCount;
      finishedSize = totalSize;
    }
    final filesStr = "$handledFileCount/$fileCount files";
    final sizeStr = totalSize > 0 ? readableFileSize(totalSize.toDouble()) : "";
    final sizePercentStr = totalSize > 0 && finishedSize > 0
        ? "${readableFileSize(finishedSize.toDouble())} / ${readableFileSize(totalSize.toDouble())}"
        : "";
    if (type == JobType.deleteFile) {
      return display();
    } else if (type == JobType.deleteDir) {
      var res = '';
      if (state == JobState.done || state == JobState.error) {
        res = display();
      }
      if (filesStr.isNotEmpty) {
        if (res.isNotEmpty) {
          res += " ";
        }
        res += filesStr;
      }

      if (sizeStr.isNotEmpty) {
        if (res.isNotEmpty) {
          res += ", ";
        }
        res += sizeStr;
      }
      return res;
    } else if (type == JobType.transfer) {
      var res = "";
      if (state != JobState.inProgress && state != JobState.none) {
        res += display();
      }
      if (filesStr.isNotEmpty) {
        if (res.isNotEmpty) {
          res += ", ";
        }
        res += filesStr;
      }
      if (sizeStr.isNotEmpty && state != JobState.inProgress) {
        if (res.isNotEmpty) {
          res += ", ";
        }
        res += sizeStr;
      }
      if (sizePercentStr.isNotEmpty && state == JobState.inProgress) {
        if (res.isNotEmpty) {
          res += ", ";
        }
        res += sizePercentStr;
      }
      return res;
    }
    return '';
  }
}

class JobResultListener<T> {
  Completer<T>? _completer;
  Timer? _timer;
  final int _timeoutSecond = 5;

  bool get isListening => _completer != null;

  clear() {
    if (_completer != null) {
      _timer?.cancel();
      _timer = null;
      _completer!.completeError("Cancel manually");
      _completer = null;
      return;
    }
  }

  Future<T> start() {
    if (_completer != null) return Future.error("Already start listen");
    _completer = Completer();
    _timer = Timer(Duration(seconds: _timeoutSecond), () {
      if (!_completer!.isCompleted) {
        _completer!.completeError("Time out");
      }
      _completer = null;
    });
    return _completer!.future;
  }

  complete(T res) {
    if (_completer != null) {
      _timer?.cancel();
      _timer = null;
      _completer!.complete(res);
      _completer = null;
      return;
    }
  }
}
