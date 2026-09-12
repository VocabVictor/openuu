part of 'file_model.dart';

class _RemoteReadTask {
  final bool includeHidden;
  final Completer<FileDirectory> completer = Completer<FileDirectory>();
  final Completer<void> released = Completer<void>();
  late final Timer timer;

  _RemoteReadTask(this.includeHidden);
}

class FileFetcher {
  // Map<String,Completer<FileDirectory>> localTasks = {}; // now we only use read local dir sync
  final Map<String, _RemoteReadTask> _remoteReadTasks = {};
  Map<String, Completer<List<FileDirectory>>> remoteEmptyDirsTasks = {};
  Map<int, Completer<FileDirectory>> readRecursiveTasks = {};
  int _remoteSessionGeneration = 0;

  final GetSessionID getSessionID;
  final ReadRemoteDirectory _readRemoteDirectory;
  SessionID get sessionId => getSessionID();

  FileFetcher(this.getSessionID, {ReadRemoteDirectory? readRemoteDirectory})
      : _readRemoteDirectory = readRemoteDirectory ??
            ((sessionId, path, includeHidden) => bind.sessionReadRemoteDir(
                sessionId: sessionId,
                path: path,
                includeHidden: includeHidden));

  bool hasPendingRemoteRead(String path) => _remoteReadTasks.containsKey(path);

  bool isLikelyRemoteHomeResponse(String path) =>
      _remoteReadTasks.isEmpty ||
      (_remoteReadTasks.length == 1 &&
          hasPendingRemoteRead("") &&
          !hasPendingRemoteRead(path));

  void beginRemoteSession() {
    _remoteSessionGeneration++;
    final pendingTasks = _remoteReadTasks.entries.toList(growable: false);
    for (final entry in pendingTasks) {
      final task = entry.value;
      if (!_removeRemoteReadTask(entry.key, task)) continue;
      task.completer.completeError(StateError(_kRemoteSessionChangedError));
    }
  }

  _RemoteReadTask _registerRemoteReadTask(String path, bool includeHidden) {
    if (hasPendingRemoteRead(path)) {
      throw "Failed to registerReadTask, already have same read job";
    }
    final task = _RemoteReadTask(includeHidden);
    _remoteReadTasks[path] = task;
    task.timer = Timer(_kRemoteReadDirTimeout, () {
      if (!_removeRemoteReadTask(path, task)) return;
      task.completer.completeError("Failed to read dir, timeout");
    });
    return task;
  }

  bool _removeRemoteReadTask(String path, _RemoteReadTask task) {
    if (!identical(_remoteReadTasks[path], task)) return false;
    _remoteReadTasks.remove(path);
    task.timer.cancel();
    task.released.complete();
    return true;
  }

  bool _completeRemoteReadTask(String path, FileDirectory directory) {
    final task = _remoteReadTasks[path];
    if (task == null || !_removeRemoteReadTask(path, task)) return false;
    task.completer.complete(directory);
    return true;
  }

  Future<List<FileDirectory>> registerReadEmptyDirsTask(
      bool isLocal, String path) {
    // final jobs = isLocal?localJobs:remoteJobs; // maybe we will use read local dir async later
    final tasks = remoteEmptyDirsTasks; // bypass now
    if (tasks.containsKey(path)) {
      throw "Failed to registerReadEmptyDirsTask, already have same read job";
    }
    final c = Completer<List<FileDirectory>>();
    tasks[path] = c;

    Timer(Duration(seconds: 2), () {
      tasks.remove(path);
      if (c.isCompleted) return;
      c.completeError("Failed to read empty dirs, timeout");
    });
    return c.future;
  }

  Future<FileDirectory> registerReadRecursiveTask(int actID) {
    final tasks = readRecursiveTasks;
    if (tasks.containsKey(actID)) {
      throw "Failed to registerRemoveTask, already have same ReadRecursive job";
    }
    final c = Completer<FileDirectory>();
    tasks[actID] = c;

    Timer(Duration(seconds: 2), () {
      tasks.remove(actID);
      if (c.isCompleted) return;
      c.completeError("Failed to read dir, timeout");
    });
    return c.future;
  }

  tryCompleteEmptyDirsTask(String? msg, String? isLocalStr) {
    if (msg == null || isLocalStr == null) return;
    late final Map<String, Completer<List<FileDirectory>>> tasks;
    try {
      final map = jsonDecode(msg);
      final String path = map["path"];
      final List<dynamic> fdJsons = map["empty_dirs"];
      final List<FileDirectory> fds =
          fdJsons.map((fdJson) => FileDirectory.fromJson(fdJson)).toList();

      tasks = remoteEmptyDirsTasks;
      final completer = tasks.remove(path);

      completer?.complete(fds);
    } catch (e) {
      debugPrint("tryCompleteJob err: $e");
    }
  }

  tryCompleteTask(String? msg, String? isLocalStr) {
    if (msg == null || isLocalStr == null) return;
    try {
      final fd = FileDirectory.fromJson(jsonDecode(msg));
      if (fd.id > 0) {
        // fd.id > 0 is result for read recursive
        final completer = readRecursiveTasks.remove(fd.id);
        completer?.complete(fd);
        return;
      }
      if (isLocalStr == "false" && fd.path.isNotEmpty) {
        if (_completeRemoteReadTask(fd.path, fd)) {
          return;
        }
        // A Home request uses an empty path but returns its resolved path.
        if (isLikelyRemoteHomeResponse(fd.path)) {
          _completeRemoteReadTask("", fd);
        }
      }
    } catch (e) {
      debugPrint("tryCompleteJob err: $e");
    }
  }

  bool tryCompleteRemoteTaskWithError(String error) {
    if (_remoteReadTasks.length != 1) return false;
    final entry = _remoteReadTasks.entries.single;
    final task = entry.value;
    if (!_removeRemoteReadTask(entry.key, task)) return false;
    task.completer.completeError(error);
    return true;
  }

  // Complete a pending recursive read task with an error.
  // See FileModel.handleJobError() for why this is necessary.
  void tryCompleteRecursiveTaskWithError(int id, String error) {
    final completer = readRecursiveTasks.remove(id);
    if (completer != null && !completer.isCompleted) {
      completer.completeError(error);
    }
  }

  Future<List<FileDirectory>> readEmptyDirs(
      String path, bool isLocal, bool showHidden) async {
    try {
      if (isLocal) {
        final res = await bind.sessionReadLocalEmptyDirsRecursiveSync(
            sessionId: sessionId, path: path, includeHidden: showHidden);

        final List<dynamic> fdJsons = jsonDecode(res);

        final List<FileDirectory> fds =
            fdJsons.map((fdJson) => FileDirectory.fromJson(fdJson)).toList();
        return fds;
      } else {
        await bind.sessionReadRemoteEmptyDirsRecursiveSync(
            sessionId: sessionId, path: path, includeHidden: showHidden);
        return registerReadEmptyDirsTask(isLocal, path);
      }
    } catch (e) {
      return Future.error(e);
    }
  }

  Future<FileDirectory> fetchDirectory(
      String path, bool isLocal, bool showHidden) async {
    try {
      if (isLocal) {
        final res = await bind.sessionReadLocalDirSync(
            sessionId: sessionId, path: path, showHidden: showHidden);
        final fd = FileDirectory.fromJson(jsonDecode(res));
        return fd;
      } else {
        final remoteSessionGeneration = _remoteSessionGeneration;
        final pendingTask = _remoteReadTasks[path];
        if (pendingTask != null) {
          if (pendingTask.includeHidden == showHidden) {
            return pendingTask.completer.future;
          }
          await pendingTask.released.future;
          if (remoteSessionGeneration != _remoteSessionGeneration) {
            throw StateError(_kRemoteSessionChangedError);
          }
          return fetchDirectory(path, isLocal, showHidden);
        }
        final task = _registerRemoteReadTask(path, showHidden);
        unawaited(Future<void>.sync(
                () => _readRemoteDirectory(sessionId, path, showHidden))
            .catchError((Object error, StackTrace stackTrace) {
          if (!_removeRemoteReadTask(path, task)) return;
          task.completer.completeError(error, stackTrace);
        }));
        return task.completer.future;
      }
    } catch (e) {
      return Future.error(e);
    }
  }

  Future<FileDirectory> fetchDirectoryRecursiveToRemove(
      int actID, String path, bool isLocal, bool showHidden) async {
    // TODO test Recursive is show hidden default?
    try {
      await bind.sessionReadDirToRemoveRecursive(
          sessionId: sessionId,
          actId: actID,
          path: path,
          isRemote: !isLocal,
          showHidden: showHidden);
      return registerReadRecursiveTask(actID);
    } catch (e) {
      return Future.error(e);
    }
  }
}

const _kRemoteReadDirTimeout = Duration(seconds: 30);

const _kRemoteSessionChangedError =
    'Remote directory read cancelled because the session changed';
