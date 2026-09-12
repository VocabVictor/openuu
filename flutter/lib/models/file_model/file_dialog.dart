part of 'file_model.dart';

/// Define a general queue which can accepts different dialog type.
///
/// [Visibility]
/// The `_FileDialogType` and `_DialogEvent` are invisible for other models.
enum FileDialogType { overwrite, unknown }

class _FileDialogEvent extends BaseEvent<FileDialogType, Map<String, dynamic>> {
  WeakReference<FileModel> fileModel;
  bool? _overrideConfirm;
  bool _skip = false;

  _FileDialogEvent(this.fileModel, super.type, super.data);

  void setOverrideConfirm(bool? confirm) {
    _overrideConfirm = confirm;
  }

  void setSkip(bool skip) {
    _skip = skip;
  }

  @override
  EventCallback<Map<String, dynamic>>? findCallback(FileDialogType type) {
    final model = fileModel.target;
    if (model == null) {
      return null;
    }
    switch (type) {
      case FileDialogType.overwrite:
        return (data) async {
          return await model.overrideFileConfirm(data,
              overrideConfirm: _overrideConfirm, skip: _skip);
        };
      default:
        debugPrint("Unknown event type: $type with $data");
        return null;
    }
  }
}

class FileDialogEventLoop
    extends BaseEventLoop<FileDialogType, Map<String, dynamic>> {
  int? _batchId;
  bool? _overrideConfirm;
  bool _skip = false;

  @override
  Future<void> onPreConsume(
      BaseEvent<FileDialogType, Map<String, dynamic>> evt) async {
    final event = evt as _FileDialogEvent;
    final model = event.fileModel.target;
    final jobId = int.tryParse(evt.data['id']?.toString() ?? '');
    final batchId = model == null || jobId == null
        ? null
        : model.jobController.transferConflictBatchId(jobId);
    final keepRemembered = model != null &&
        model.jobController.isTransferConflictRememberBatch(batchId);
    // The loop only preloads the remembered batch choice. The model updates it
    // after the user answers the current overwrite dialog.
    if (_batchId != batchId && !keepRemembered) {
      _batchId = batchId;
      _overrideConfirm = null;
      _skip = false;
    } else {
      _batchId = batchId;
    }
    if (keepRemembered) {
      _overrideConfirm =
          model.jobController.transferConflictRememberOverrideConfirm(batchId);
      _skip = _overrideConfirm == null;
    }
    event.setOverrideConfirm(_overrideConfirm);
    event.setSkip(_skip);
    debugPrint(
        "FileDialogEventLoop: consuming<jobId: ${evt.data['id']} batchId: $_batchId overrideConfirm: $_overrideConfirm, skip: $_skip>");
  }

  @override
  Future<void> onEventsClear() {
    _batchId = null;
    _overrideConfirm = null;
    _skip = false;
    return super.onEventsClear();
  }

  void setOverrideConfirm(bool? confirm) {
    _overrideConfirm = confirm;
  }

  void setSkip(bool skip) {
    _skip = skip;
  }
}
