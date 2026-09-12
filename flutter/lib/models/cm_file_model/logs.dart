part of 'cm_file_model.dart';

class CmFileLog {
  JobState state = JobState.none;
  var id = 0;
  var speed = 0.0;
  var finishedSize = 0;
  var totalSize = 0;
  CmFileAction action = CmFileAction.none;
  var fileName = "";
  var err = "";
  int lastTransferredSize = 0;

  String display() {
    if (state == JobState.done && err == "skipped") {
      return translate("Skipped");
    }
    return state.display();
  }

  bool isTransfer() {
    return action == CmFileAction.remoteToLocal ||
        action == CmFileAction.localToRemote;
  }
}

class TransferJobSerdeData {
  int connId;
  int id;
  String path;
  bool isRemote;
  int totalSize;
  int finishedSize;
  int transferred;
  bool done;
  bool cancel;
  String error;

  TransferJobSerdeData({
    required this.connId,
    required this.id,
    required this.path,
    required this.isRemote,
    required this.totalSize,
    required this.finishedSize,
    required this.transferred,
    required this.done,
    required this.cancel,
    required this.error,
  });

  TransferJobSerdeData.fromJson(dynamic d)
      : this(
          connId: d['connId'] ?? 0,
          id: int.tryParse(d['id'].toString()) ?? 0,
          path: d['dataSource'] ?? '',
          isRemote: d['isRemote'] ?? false,
          totalSize: d['totalSize'] ?? 0,
          finishedSize: d['finishedSize'] ?? 0,
          transferred: d['transferred'] ?? 0,
          done: d['done'] ?? false,
          cancel: d['cancel'] ?? false,
          error: d['error'] ?? '',
        );
}

class FileActionLog {
  int id = 0;
  int connId = 0;
  String path = '';
  bool dir = false;

  FileActionLog({
    required this.connId,
    required this.id,
    required this.path,
    required this.dir,
  });

  FileActionLog.fromJson(dynamic d)
      : this(
          connId: d['connId'] ?? 0,
          id: d['id'] ?? 0,
          path: d['path'] ?? '',
          dir: d['dir'] ?? false,
        );
}
