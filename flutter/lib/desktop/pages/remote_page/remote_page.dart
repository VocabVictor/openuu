import 'dart:async';
import '../../widgets/quick_launch.dart';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/scheduler.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';
import 'package:flutter_hbb/models/state_model.dart';

import '../../../consts.dart';
import '../../../common/widgets/overlay.dart';
import '../../../common/widgets/remote_input.dart';
import '../../../common.dart';
import '../../../common/widgets/dialog.dart';
import '../../../common/widgets/toolbar.dart';
import '../../../models/model.dart';
import '../../../models/input_model.dart';
import '../../../models/platform_model.dart';
import '../../../common/shared_state.dart';
import '../../../utils/image.dart';
import '../../widgets/remote_toolbar.dart';
import '../../widgets/desktop_preview.dart';
import '../../widgets/kb_layout_type_chooser.dart';
import '../../widgets/tabbar_widget.dart';
import '../macos_full_screen_focus_recovery.dart';

import 'package:flutter_hbb/native/custom_cursor.dart'
    if (dart.library.html) 'package:flutter_hbb/web/custom_cursor.dart';
part 'view.dart';
part 'macos.dart';
part 'body.dart';
part 'widgets.dart';

final SimpleWrapper<bool> _firstEnterImage = SimpleWrapper(false);

// Used to skip session close if "move to new window" is clicked.
final Map<String, bool> closeSessionOnDispose = {};

class RemotePage extends StatefulWidget {
  RemotePage({
    Key? key,
    required this.id,
    required this.toolbarState,
    this.sessionId,
    this.tabWindowId,
    this.password,
    this.display,
    this.displays,
    this.tabController,
    this.switchUuid,
    this.forceRelay,
    this.viewOnly = false,
    this.quickLaunch,
    this.isSharedPassword,
  }) : super(key: key) {
    initSharedStates(id);
  }

  final String id;
  final SessionID? sessionId;
  final int? tabWindowId;
  final int? display;
  final List<int>? displays;
  final String? password;
  final ToolbarState toolbarState;
  final String? switchUuid;
  final bool? forceRelay;
  final bool viewOnly;
  final String? quickLaunch;
  final bool? isSharedPassword;
  final SimpleWrapper<State<RemotePage>?> _lastState = SimpleWrapper(null);
  final DesktopTabController? tabController;

  FFI get ffi => (_lastState.value! as _RemotePageState)._ffi;

  void releaseMacOSInputForTabTransfer() {
    if (!isMacOS) return;
    // Release before removing the source tab. Its delayed disposal must not
    // disable a native keyboard hook already acquired by the destination page.
    (_lastState.value! as _RemotePageState)._releaseMacOSRemoteInput();
  }

  @override
  State<RemotePage> createState() {
    final state = _RemotePageState(id);
    _lastState.value = state;
    return state;
  }
}

class _RemotePageState extends State<RemotePage>
    with
        AutomaticKeepAliveClientMixin,
        MultiWindowListener,
        WidgetsBindingObserver,
        TickerProviderStateMixin {
  void _setState(VoidCallback fn) => setState(fn);
  Timer? _timer;
  String keyboardMode = "legacy";
  bool _isWindowBlur = false;
  // Known macOS remote-input trade-offs (kept simple intentionally):
  // 1. Dialogs rely on FocusNode loss plus middleBlocked, not mirrored dialog
  //    state. Reproduce: activate remote input, open a dialog, then type.
  // 2. Delayed fullscreen recovery can race a local-control focus change; no
  //    owner state is added. Reproduce: focus the toolbar during a Space switch.
  // 3. Input-source switching releases native input without updating this
  //    page's cache. Reproduce: switch sources, then type before and after
  //    clicking the remote image; the click reasserts input.
  // These latches compensate for out-of-order macOS focus events. Treat them
  // as coupled when changing a transition or _syncMacOSKeyboardGrab().
  AppLifecycleState? _macOSLifecycleState;
  bool _macOSLocalFocusLost = false;
  bool _macOSInputActive = false;
  bool _macOSInputSuppressed = false;
  final _macOSFullScreenFocusRecovery = MacOSFullScreenFocusRecovery();
  bool _macOSExplicitFocusRequestPending = false;
  StreamSubscription<DesktopTabState>? _tabStateSubscription;
  final _cursorOverImage = false.obs;
  late RxBool _showRemoteCursor;
  late RxBool _zoomCursor;
  late RxBool _remoteCursorMoved;
  late RxBool _keyboardEnabled;
  final _uniqueKey = UniqueKey();

  var _blockableOverlayState = BlockableOverlayState();

  final FocusNode _rawKeyFocusNode = FocusNode(debugLabel: "rawkeyFocusNode");

  // Debounce timer for pointer lock center updates during window events.
  // Uses kDefaultPointerLockCenterThrottleMs from consts.dart for the duration.
  Timer? _pointerLockCenterDebounceTimer;

  // We need `_instanceIdOnEnterOrLeaveImage4Toolbar` together with `_onEnterOrLeaveImage4Toolbar`
  // to identify the toolbar instance and its callback function.
  int? _instanceIdOnEnterOrLeaveImage4Toolbar;
  Function(bool)? _onEnterOrLeaveImage4Toolbar;

  late FFI _ffi;
  Worker? _waylandKeyboardModeWorker;
  bool _waylandKeyboardModeNormalized = false;
  bool _waylandKeyboardModeNormalizing = false;

  SessionID get sessionId => _ffi.sessionId;

  _RemotePageState(String id) {
    _initStates(id);
  }

  void _initStates(String id) {
    _zoomCursor = PeerBoolOption.find(id, kOptionZoomCursor);
    _showRemoteCursor = ShowRemoteCursorState.find(id);
    _keyboardEnabled = KeyboardEnabledState.find(id);
    _remoteCursorMoved = RemoteCursorMovedState.find(id);
  }

  @override
  void initState() {
    super.initState();
    _ffi = FFI(widget.sessionId);
    if (isMacOS) {
      // SchedulerBinding.instance.lifecycleState is null in the first connection in a new window.
      _macOSLifecycleState = SchedulerBinding.instance.lifecycleState;
      WidgetsBinding.instance.addObserver(this);
      _tabStateSubscription =
          widget.tabController?.state.listen(_onMacOSTabStateChanged);
    }
    Get.put<FFI>(_ffi, tag: widget.id);
    _ffi.imageModel.addCallbackOnFirstImage((String peerId) {
      _ffi.canvasModel.activateLocalCursor();
      showKBLayoutTypeChooserIfNeeded(
          _ffi.ffiModel.pi.platform, _ffi.dialogManager);
      _ffi.recordingModel
          .updateStatus(bind.sessionGetIsRecording(sessionId: _ffi.sessionId));
    });
    _ffi.canvasModel.initializeEdgeScrollFallback(this);
    if (widget.quickLaunch != null) {
      var shown = false;
      void openQuickLaunch() {
        if (shown || !mounted || !_ffi.ffiModel.pi.isSet.isTrue) return;
        shown = true;
        _ffi.ffiModel.removeListener(openQuickLaunch);
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted) showQuickLaunch(context, _ffi, widget.quickLaunch!);
        });
      }
      _ffi.ffiModel.addListener(openQuickLaunch);
    }
    _ffi.start(
      widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      switchUuid: widget.switchUuid,
      forceRelay: widget.forceRelay,
      viewOnly: widget.viewOnly,
      tabWindowId: widget.tabWindowId,
      display: widget.display,
      displays: widget.displays,
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });
    WakelockManager.enable(_uniqueKey);

    _ffi.ffiModel.updateEventListener(sessionId, widget.id);
    _ffi.qualityMonitorModel.checkShowQualityMonitor(sessionId);
    _ffi.dialogManager.loadMobileActionsOverlayVisible();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      // Session option should be set after models.dart/FFI.start
      _showRemoteCursor.value = bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: 'show-remote-cursor');
      _zoomCursor.value = bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: kOptionZoomCursor);
    });
    DesktopMultiWindow.addListener(this);
    // if (!_isCustomCursorInited) {
    //   customCursorController.registerNeedUpdateCursorCallback(
    //       (String? lastKey, String? currentKey) async {
    //     if (_firstEnterImage.value) {
    //       _firstEnterImage.value = false;
    //       return true;
    //     }
    //     return lastKey == null || lastKey != currentKey;
    //   });
    //   _isCustomCursorInited = true;
    // }

    _blockableOverlayState.applyFfi(_ffi);
    // Call onSelected in post frame callback, since we cannot guarantee that the callback will not call setState.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.tabController?.onSelected?.call(widget.id);
    });

    // Register callback to cancel debounce timer when relative mouse mode is disabled
    _ffi.inputModel.onRelativeMouseModeDisabled =
        _cancelPointerLockCenterDebounceTimer;

    _waylandKeyboardModeWorker = ever(_ffi.ffiModel.pi.isSet, (bool isSet) {
      if (isSet) {
        unawaited(_normalizeWaylandKeyboardModeIfNeeded());
      }
    });
    if (_ffi.ffiModel.pi.isSet.value) {
      unawaited(_normalizeWaylandKeyboardModeIfNeeded());
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (!isMacOS || _macOSLifecycleState == state) return;
    _macOSLifecycleState = state;
    if (state == AppLifecycleState.resumed) {
      _syncMacOSKeyboardGrab(reassert: true);
    } else if (_macOSInputActive) {
      _ffi.inputModel.enterOrLeave(false);
      _macOSInputActive = false;
    }

    final generation = _macOSFullScreenFocusRecovery.pendingGeneration;
    if (generation == null) return;
    if (state == AppLifecycleState.inactive ||
        state == AppLifecycleState.resumed) {
      _scheduleMacOSKeyboardAfterFullScreen(generation: generation);
    } else if (state == AppLifecycleState.paused ||
        state == AppLifecycleState.detached) {
      _macOSFullScreenFocusRecovery.cancel();
    }
  }

  @override
  void onWindowBlur() {
    super.onWindowBlur();
    // On windows, we use `focus` way to handle keyboard better.
    // Now on Linux, there's some rdev issues which will break the input.
    // We disable the `focus` way for Linux temporarily.
    if (isWindows || isMacOS) {
      _isWindowBlur = true;
    }
    if (isMacOS) {
      _macOSFullScreenFocusRecovery.cancel();
      // A blur or Space switch may not emit PointerExit, so cursor state alone
      // cannot prevent the old remote surface from reclaiming the keyboard.
      _macOSLocalFocusLost = true;
    }
    if (isWindows) {
      // unfocus the primary-focus when the whole window is lost focus,
      // and let OS to handle events instead.
      _rawKeyFocusNode.unfocus();
    }
    stateGlobal.isFocused.value = false;
    _syncMacOSKeyboardGrab();

    // When window loses focus, temporarily release relative mouse mode constraints
    // to allow user to interact with other applications normally.
    // The cursor will be re-hidden and re-centered when window regains focus.
    if (_ffi.inputModel.relativeMouseMode.value) {
      _ffi.inputModel.onWindowBlur();
    }
  }

  @override
  void onWindowFocus() {
    super.onWindowFocus();
    // See [onWindowBlur].
    if (isWindows || isMacOS) {
      _isWindowBlur = false;
    }
    if (isMacOS) stateGlobal.getInputSource(force: true);
    stateGlobal.isFocused.value = true;

    // Normal macOS windows wait for PointerEnter or PointerDown. A focused
    // fullscreen Space queues delayed recovery; if this window blurs again, the
    // pending recovery is cancelled before native input can reactivate.
    // Regression: switch directly between fullscreen remote Spaces without
    // moving or clicking; only the newly focused session may receive input.
    if (isMacOS &&
        stateGlobal.fullscreen.isTrue &&
        !_ffi.inputModel.relativeMouseMode.value) {
      // Native window focus is authoritative when a secondary engine retains a
      // stale hidden lifecycle state after its fullscreen Space becomes visible.
      _queueMacOSKeyboardAfterFullScreen(allowHiddenLifecycle: true);
    }

    // Refocus without PointerEnter: the cursor already hovers the image when
    // focus returns (Alt+Tab, taskbar), so enterView() never fires again.
    if (isWindows &&
        _cursorOverImage.value &&
        _windowsCanFocusRemoteInput &&
        !_rawKeyFocusNode.hasFocus) {
      _rawKeyFocusNode.requestFocus();
    }

    // Restore relative mouse mode constraints when window regains focus.
    if (_ffi.inputModel.relativeMouseMode.value) {
      if (isMacOS) {
        // Native relative mode retains pointer capture and does not emit
        // PointerEnter after window focus returns. Restore both latches unless
        // a local overlay still owns input.
        if (_blockableOverlayState.middleBlocked.isFalse) {
          _cursorOverImage.value = true;
          _macOSLocalFocusLost = false;
        }
      } else if (!isWindows || _windowsCanFocusRemoteInput) {
        _rawKeyFocusNode.requestFocus();
      }
      _ffi.inputModel.onWindowFocus();
    }
    _syncMacOSKeyboardGrab(reassert: true, allowInactiveLifecycle: true);
  }

  @override
  void onWindowRestore() {
    super.onWindowRestore();
    // On windows, we use `onWindowRestore` way to handle window restore from
    // a minimized state.
    if (isWindows) {
      _isWindowBlur = false;
    }
    WakelockManager.enable(_uniqueKey);
    // Update pointer lock center when window is restored
    _updatePointerLockCenterIfNeeded();
  }

  // When the window is unminimized, onWindowMaximize or onWindowRestore can be called when the old state was maximized or not.
  @override
  void onWindowMaximize() {
    super.onWindowMaximize();
    WakelockManager.enable(_uniqueKey);
    // Update pointer lock center when window is maximized
    _updatePointerLockCenterIfNeeded();
  }

  @override
  void onWindowResize() {
    super.onWindowResize();
    // Update pointer lock center when window is resized
    _updatePointerLockCenterIfNeeded();
  }

  @override
  void onWindowMove() {
    super.onWindowMove();
    // Update pointer lock center when window is moved
    _updatePointerLockCenterIfNeeded();
  }

  /// Update pointer lock center with debouncing to avoid excessive updates
  /// during rapid window move/resize events.
  void _updatePointerLockCenterIfNeeded() {
    if (!_ffi.inputModel.relativeMouseMode.value) return;

    // Cancel any pending update and schedule a new one (debounce pattern)
    _pointerLockCenterDebounceTimer?.cancel();
    _pointerLockCenterDebounceTimer = Timer(
      const Duration(milliseconds: kDefaultPointerLockCenterThrottleMs),
      () {
        if (!mounted) return;
        if (_ffi.inputModel.relativeMouseMode.value) {
          _ffi.inputModel.updatePointerLockCenter();
        }
      },
    );
  }

  @override
  void onWindowMinimize() {
    super.onWindowMinimize();
    WakelockManager.disable(_uniqueKey);
    if (isMacOS) {
      _macOSFullScreenFocusRecovery.cancel();
      _isWindowBlur = true;
      _cursorOverImage.value = false;
      stateGlobal.isFocused.value = false;
      _syncMacOSKeyboardGrab();
    }
    // Release cursor constraints when minimized
    if (_ffi.inputModel.relativeMouseMode.value) {
      _ffi.inputModel.onWindowBlur();
    }
  }

  @override
  void onWindowEnterFullScreen() {
    super.onWindowEnterFullScreen();
    if (isMacOS) {
      stateGlobal.setFullscreen(true);
      _queueMacOSKeyboardAfterFullScreen();
    }
  }

  @override
  void onWindowLeaveFullScreen() {
    super.onWindowLeaveFullScreen();
    if (isMacOS) {
      stateGlobal.setFullscreen(false);
      _queueMacOSKeyboardAfterFullScreen();
    }
  }

  @override
  Future<void> dispose() async {
    final closeSession = closeSessionOnDispose.remove(widget.id) ?? true;

    // https://github.com/flutter/flutter/issues/64935
    if (isMacOS) {
      // Tab moves release before transfer to avoid a late retained-session leave.
      if (closeSession) {
        _releaseMacOSRemoteInput();
      }
      _tabStateSubscription?.cancel();
      WidgetsBinding.instance.removeObserver(this);
    }
    super.dispose();
    debugPrint("REMOTE PAGE dispose session $sessionId ${widget.id}");

    // Defensive cleanup: ensure host system-key propagation is reset even if
    // MouseRegion.onExit never fired (e.g., tab closed while cursor inside).
    if (!isWeb) bind.hostStopSystemKeyPropagate(stopped: true);

    _pointerLockCenterDebounceTimer?.cancel();
    _pointerLockCenterDebounceTimer = null;
    _waylandKeyboardModeWorker?.dispose();
    // Clear callback reference to prevent memory leaks and stale references
    _ffi.inputModel.onRelativeMouseModeDisabled = null;
    // Relative mouse mode cleanup is centralized in FFI.close(closeSession: ...).
    _ffi.textureModel.onRemotePageDispose(closeSession);
    if (closeSession && !isMacOS) {
      // ensure we leave this session, this is a double check
      // enterOrLeave() is already called previously in _releaseMacOSRemoteInput() for macOS.
      _ffi.inputModel.enterOrLeave(false);
    }
    DesktopMultiWindow.removeListener(this);
    _ffi.dialogManager.hideMobileActionsOverlay();
    _ffi.imageModel.disposeImage();
    _ffi.cursorModel.disposeImages();
    _rawKeyFocusNode.dispose();
    if (closeSession) {
      clearWaylandKeyboardPromptSuppressedForConnection(sessionId.toString());
    }
    await _ffi.close(closeSession: closeSession);
    _timer?.cancel();
    _ffi.dialogManager.dismissAll();
    if (closeSession) {
      await SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
          overlays: SystemUiOverlay.values);
    }
    WakelockManager.disable(_uniqueKey);
    await Get.delete<FFI>(tag: widget.id);
    removeSharedStates(widget.id);
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return WillPopScope(
        onWillPop: () async {
          clientClose(sessionId, _ffi);
          return false;
        },
        child: MultiProvider(providers: [
          ChangeNotifierProvider.value(value: _ffi.ffiModel),
          ChangeNotifierProvider.value(value: _ffi.imageModel),
          ChangeNotifierProvider.value(value: _ffi.cursorModel),
          ChangeNotifierProvider.value(value: _ffi.canvasModel),
          ChangeNotifierProvider.value(value: _ffi.recordingModel),
        ], child: buildBody(context)));
  }

  @override
  bool get wantKeepAlive => true;
}

class ImagePaint extends StatefulWidget {
  final FFI ffi;
  final String id;
  final RxBool zoomCursor;
  final RxBool cursorOverImage;
  final RxBool keyboardEnabled;
  final RxBool remoteCursorMoved;
  final Widget Function(Widget)? listenerBuilder;

  ImagePaint(
      {Key? key,
      required this.ffi,
      required this.id,
      required this.zoomCursor,
      required this.cursorOverImage,
      required this.keyboardEnabled,
      required this.remoteCursorMoved,
      this.listenerBuilder})
      : super(key: key);

  @override
  State<StatefulWidget> createState() => _ImagePaintState();
}

class _ImagePaintState extends State<ImagePaint> {
  bool _lastRemoteCursorMoved = false;

  String get id => widget.id;
  RxBool get zoomCursor => widget.zoomCursor;
  RxBool get cursorOverImage => widget.cursorOverImage;
  RxBool get keyboardEnabled => widget.keyboardEnabled;
  RxBool get remoteCursorMoved => widget.remoteCursorMoved;
  Widget Function(Widget)? get listenerBuilder => widget.listenerBuilder;

  @override
  Widget build(BuildContext context) {
    final m = Provider.of<ImageModel>(context);
    var c = Provider.of<CanvasModel>(context);
    final s = c.scale;

    bool isViewAdaptive() => c.viewStyle.style == kRemoteViewStyleAdaptive;
    bool isViewOriginal() => c.viewStyle.style == kRemoteViewStyleOriginal;

    mouseRegion({child}) => Obx(() {
          double getCursorScale() {
            var c = Provider.of<CanvasModel>(context);
            var cursorScale = 1.0;
            if (isWindows) {
              // debug win10
              if (zoomCursor.value && isViewAdaptive()) {
                cursorScale = s * c.devicePixelRatio;
              }
            } else {
              if (zoomCursor.value || isViewOriginal()) {
                cursorScale = s;
              }
            }
            return cursorScale;
          }

          return MouseRegion(
              cursor: cursorOverImage.isTrue
                  ? c.cursorEmbedded
                      ? SystemMouseCursors.none
                      // Hide cursor when relative mouse mode is active
                      : widget.ffi.inputModel.relativeMouseMode.value
                          ? SystemMouseCursors.none
                          : keyboardEnabled.isTrue
                              ? (() {
                                  if (remoteCursorMoved.isTrue) {
                                    _lastRemoteCursorMoved = true;
                                    return SystemMouseCursors.none;
                                  } else {
                                    if (_lastRemoteCursorMoved) {
                                      _lastRemoteCursorMoved = false;
                                      _firstEnterImage.value = true;
                                    }
                                    return _buildCustomCursor(
                                        context, getCursorScale());
                                  }
                                }())
                              : _buildDisabledCursor(context, getCursorScale())
                  : MouseCursor.defer,
              onHover: (evt) {},
              child: child);
        });
    if (c.imageOverflow.isTrue && c.scrollStyle != ScrollStyle.scrollauto) {
      final paintWidth = c.getDisplayWidth() * s;
      final paintHeight = c.getDisplayHeight() * s;
      final paintSize = Size(paintWidth, paintHeight);
      final paintWidget =
          m.useTextureRender || widget.ffi.ffiModel.pi.forceTextureRender
              ? _BuildPaintTextureRender(
                  c, s, Offset.zero, paintSize, isViewOriginal())
              : _buildScrollbarNonTextureRender(m, paintSize, s);
      return NotificationListener<ScrollNotification>(
          onNotification: (notification) {
            c.updateScrollPercent();
            return false;
          },
          child: mouseRegion(
            child: Obx(() => _buildCrossScrollbarFromLayout(
                  context,
                  _buildListener(paintWidget),
                  c.size,
                  paintSize,
                  c.scrollHorizontal,
                  c.scrollVertical,
                )),
          ));
    } else {
      if (c.size.width > 0 && c.size.height > 0) {
        final paintWidget =
            m.useTextureRender || widget.ffi.ffiModel.pi.forceTextureRender
                ? _BuildPaintTextureRender(
                    c,
                    s,
                    Offset(
                      isLinux ? c.x.toInt().toDouble() : c.x,
                      isLinux ? c.y.toInt().toDouble() : c.y,
                    ),
                    c.size,
                    isViewOriginal())
                : _buildScrollAutoNonTextureRender(m, c, s);
        return mouseRegion(child: _buildListener(paintWidget));
      } else {
        return Container();
      }
    }
  }

  Widget _buildScrollbarNonTextureRender(
      ImageModel m, Size imageSize, double s) {
    return CustomPaint(
      size: imageSize,
      painter: ImagePainter(image: m.image, x: 0, y: 0, scale: s),
    );
  }

  Widget _buildScrollAutoNonTextureRender(
      ImageModel m, CanvasModel c, double s) {
    double sizeScale = s;
    if (widget.ffi.ffiModel.isPeerLinux) {
      final displays = widget.ffi.ffiModel.pi.getCurDisplays();
      if (displays.isNotEmpty) {
        sizeScale = s / displays[0].scale;
      }
    }
    return CustomPaint(
      size: Size(c.size.width, c.size.height),
      painter: ImagePainter(
          image: m.image,
          x: c.x / sizeScale,
          y: c.y / sizeScale,
          scale: sizeScale),
    );
  }

  Widget _BuildPaintTextureRender(
      CanvasModel c, double s, Offset offset, Size size, bool isViewOriginal) {
    final ffiModel = c.parent.target!.ffiModel;
    final displays = ffiModel.pi.getCurDisplays();
    final children = <Widget>[];
    final rect = ffiModel.rect;
    if (rect == null) {
      return Container();
    }
    final isPeerLinux = ffiModel.isPeerLinux;
    final curDisplay = ffiModel.pi.currentDisplay;
    for (var i = 0; i < displays.length; i++) {
      final textureId = widget.ffi.textureModel
          .getTextureId(curDisplay == kAllDisplayValue ? i : curDisplay);
      if (true) {
        // both "textureId.value != -1" and "true" seems ok
        final sizeScale = isPeerLinux ? s / displays[i].scale : s;
        children.add(Positioned(
          left: (displays[i].x - rect.left) * s + offset.dx,
          top: (displays[i].y - rect.top) * s + offset.dy,
          width: displays[i].width * sizeScale,
          height: displays[i].height * sizeScale,
          child: Obx(() => Texture(
                textureId: textureId.value,
                filterQuality:
                    isViewOriginal ? FilterQuality.none : FilterQuality.low,
              )),
        ));
      }
    }
    return SizedBox(
      width: size.width,
      height: size.height,
      child: Stack(children: children),
    );
  }

  MouseCursor _buildCustomCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = cursor.cache ?? preDefaultCursor.cache;
    return buildCursorOfCache(cursor, scale, cache);
  }

  MouseCursor _buildDisabledCursor(BuildContext context, double scale) {
    final cursor = Provider.of<CursorModel>(context);
    final cache = preForbiddenCursor.cache;
    return buildCursorOfCache(cursor, scale, cache);
  }

  Widget _buildCrossScrollbarFromLayout(
    BuildContext context,
    Widget child,
    Size layoutSize,
    Size size,
    ScrollController horizontal,
    ScrollController vertical,
  ) {
    var widget = child;
    if (layoutSize.width < size.width) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: horizontal,
          scrollDirection: Axis.horizontal,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Row(
        children: [
          Container(
            width: ((layoutSize.width - size.width) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.height < size.height) {
      widget = ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(scrollbars: false),
        child: SingleChildScrollView(
          controller: vertical,
          physics: cursorOverImage.isTrue
              ? const NeverScrollableScrollPhysics()
              : null,
          child: widget,
        ),
      );
    } else {
      widget = Column(
        children: [
          Container(
            height: ((layoutSize.height - size.height) ~/ 2).toDouble(),
          ),
          widget,
        ],
      );
    }
    if (layoutSize.width < size.width) {
      widget = RawScrollbar(
        thickness: kScrollbarThickness,
        thumbColor: Colors.grey,
        controller: horizontal,
        thumbVisibility: false,
        trackVisibility: false,
        notificationPredicate: layoutSize.height < size.height
            ? (notification) => notification.depth == 1
            : defaultScrollNotificationPredicate,
        child: widget,
      );
    }
    if (layoutSize.height < size.height) {
      widget = RawScrollbar(
        thickness: kScrollbarThickness,
        thumbColor: Colors.grey,
        controller: vertical,
        thumbVisibility: false,
        trackVisibility: false,
        child: widget,
      );
    }

    return Container(
      child: widget,
      width: layoutSize.width,
      height: layoutSize.height,
    );
  }

  Widget _buildListener(Widget child) {
    child = DesktopPreviewCapture(
      peer: id,
      ready: () => widget.ffi.ffiModel.waitForFirstImage.isFalse,
      child: child,
    );
    if (listenerBuilder != null) {
      return listenerBuilder!(child);
    } else {
      return child;
    }
  }
}
