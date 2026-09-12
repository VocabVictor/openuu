part of 'file_model.dart';

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
