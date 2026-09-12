part of 'model.dart';

/// Flutter state manager and data communication with the Rust core.
class FFI {
  bool viewOnlySession = false;
  var id = '';
  var version = '';
  var connType = ConnType.defaultConn;
  var closed = false;

  /// dialogManager use late to ensure init after main page binding [globalKey]
  late final dialogManager = OverlayDialogManager();

  late final SessionID sessionId;
  late final ImageModel imageModel; // session
  late final FfiModel ffiModel; // session
  late final CursorModel cursorModel; // session
  late final CanvasModel canvasModel; // session
  late final ServerModel serverModel; // global
  late final ChatModel chatModel; // session
  late final FileModel fileModel; // session
  late final AbModel abModel; // global
  late final GroupModel groupModel; // global
  late final UserModel userModel; // global
  late final PeerTabModel peerTabModel; // global
  late final QualityMonitorModel qualityMonitorModel; // session
  late final RecordingModel recordingModel; // session
  late final InputModel inputModel; // session
  late final ElevationModel elevationModel; // session
  late final CmFileModel cmFileModel; // cm
  late final TextureModel textureModel; //session
  late final Peers recentPeersModel; // global
  late final Peers favoritePeersModel; // global
  late final Peers lanPeersModel; // global

  // Terminal model registry for multiple terminals
  final Map<int, TerminalModel> _terminalModels = {};

  // Getter for terminal models
  Map<int, TerminalModel> get terminalModels => _terminalModels;

  FFI(SessionID? sId) {
    sessionId = sId ?? (isDesktop ? Uuid().v4obj() : _constSessionId);
    imageModel = ImageModel(WeakReference(this));
    ffiModel = FfiModel(WeakReference(this));
    cursorModel = CursorModel(WeakReference(this));
    canvasModel = CanvasModel(WeakReference(this));
    serverModel = ServerModel(WeakReference(this));
    chatModel = ChatModel(WeakReference(this));
    fileModel = FileModel(WeakReference(this));
    userModel = UserModel(WeakReference(this));
    peerTabModel = PeerTabModel(WeakReference(this));
    abModel = AbModel(WeakReference(this));
    groupModel = GroupModel(WeakReference(this));
    qualityMonitorModel = QualityMonitorModel(WeakReference(this));
    recordingModel = RecordingModel(WeakReference(this));
    inputModel = InputModel(WeakReference(this));
    elevationModel = ElevationModel(WeakReference(this));
    cmFileModel = CmFileModel(WeakReference(this));
    textureModel = TextureModel(WeakReference(this));
    recentPeersModel = Peers(
        name: PeersModelName.recent,
        loadEvent: LoadEvent.recent,
        getInitPeers: null);
    favoritePeersModel = Peers(
        name: PeersModelName.favorite,
        loadEvent: LoadEvent.favorite,
        getInitPeers: null);
    lanPeersModel = Peers(
        name: PeersModelName.lan, loadEvent: LoadEvent.lan, getInitPeers: null);
  }

  /// Mobile reuse FFI
  void mobileReset() {
    ffiModel.resetRestartReconnectState();
    ffiModel.waitForFirstImage.value = true;
    ffiModel.isRefreshing = false;
    ffiModel.waitForImageDialogShow.value = true;
    ffiModel.waitForImageTimer?.cancel();
    ffiModel.waitForImageTimer = null;
  }

}
