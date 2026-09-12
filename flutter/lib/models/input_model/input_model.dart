import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'package:flutter/foundation.dart';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../model.dart';
import '../platform_model.dart';
import '../state_model.dart';
import '../input_modifier_utils.dart';
import '../relative_mouse_model.dart';
import '../../common.dart';
import '../../consts.dart';
part 'coords.dart';
part 'pointer_event.dart';
part 'key_events.dart';
part 'keyboard.dart';
part 'mouse_move.dart';
part 'key_mouse_send.dart';
part 'touch.dart';
part 'trackpad.dart';

/// Mouse button enum.
enum MouseButtons { left, right, wheel, back, forward }

const _kMouseEventDown = 'mousedown';
const _kMouseEventUp = 'mouseup';
const _kMouseEventMove = 'mousemove';

class InputModel {
  // Side mouse button support for Linux.
  // Flutter's Linux embedder drops X11 button 8/9 events, so we capture them
  // natively via GDK and forward through the platform channel.
  static InputModel? _activeSideButtonModel;
  // Tracks per-button which model received a side button down event, so the
  // matching up event is routed there even if the pointer has left the view
  // or a different button was pressed in between.
  static final Map<MouseButtons, InputModel> _sideButtonDownModels = {};
  static bool _sideButtonChannelInitialized = false;

  /// Each Flutter engine (main window + sub-windows from desktop_multi_window)
  /// runs its own Dart isolate with its own statics. Called from initEnv()
  /// which runs per-engine, so each isolate registers its own handler tied
  /// to its own set of InputModels.
  static void initSideButtonChannel() {
    if (!isLinux) return;
    if (_sideButtonChannelInitialized) return;
    _sideButtonChannelInitialized = true;

    const channel = MethodChannel('org.rustdesk.rustdesk/side_buttons');
    channel.setMethodCallHandler((call) async {
      if (call.method == 'onSideMouseButton') {
        final args = call.arguments as Map<dynamic, dynamic>;
        final button = args['button'] as String;
        final type = args['type'] as String;
        final mb = button == 'back' ? MouseButtons.back : MouseButtons.forward;

        if (type == 'down') {
          final model = _activeSideButtonModel;
          if (model != null &&
              !(model.isViewOnly && !model.showMyCursor) &&
              model.keyboardPerm &&
              !model.isViewCamera) {
            _sideButtonDownModels[mb] = model;
            // Fire-and-forget to avoid blocking the platform channel handler.
            unawaited(model._sendMouseUnchecked(type, mb).catchError((Object e) {
              debugPrint('[InputModel] failed to send side button $type for $mb: $e');
            }));
          }
        } else {
          // Only route 'up' when we recorded the matching 'down';
          // dropping avoids sending unpaired 'up' to an unrelated session.
          // Use _sendMouseUnchecked to bypass permission checks so the
          // release always goes through even if permissions changed.
          final model = _sideButtonDownModels.remove(mb);
          if (model != null) {
            unawaited(model._sendMouseUnchecked(type, mb).catchError((Object e) {
              debugPrint('[InputModel] failed to send side button $type for $mb: $e');
            }));
          }
        }
      }
      return null;
    });
  }

  /// Clear any static references to this model (prevents stale routing).
  /// Releases any held side buttons on the peer so closing a session
  /// mid-press does not leave a stuck button.
  void disposeSideButtonTracking() {
    if (_activeSideButtonModel == this) _activeSideButtonModel = null;
    final held = _sideButtonDownModels.entries
        .where((e) => e.value == this)
        .map((e) => e.key)
        .toList();
    for (final mb in held) {
      _sideButtonDownModels.remove(mb);
      // Best-effort release; session may already be tearing down.
      unawaited(_sendMouseUnchecked('up', mb).catchError((Object e) {
        debugPrint('[InputModel] failed to release side button $mb: $e');
      }));
    }
  }

  final WeakReference<FFI> parent;
  String keyboardMode = '';

  // keyboard
  var shift = false;
  var ctrl = false;
  var alt = false;
  var command = false;

  final ToReleaseRawKeys toReleaseRawKeys = ToReleaseRawKeys();
  final ToReleaseKeys toReleaseKeys = ToReleaseKeys();

  // trackpad
  var _trackpadLastDelta = Offset.zero;
  var _stopFling = true;
  var _fling = false;
  Timer? _flingTimer;
  final _flingBaseDelay = 30;
  final _trackpadAdjustPeerLinux = 0.06;
  // This is an experience value.
  final _trackpadAdjustMacToWin = 2.50;
  // Ignore directional locking for very small deltas on both axes (including
  // tiny single-axis movement) to avoid over-filtering near zero.
  static const double _trackpadAxisNoiseThreshold = 0.2;
  // Lock to dominant axis only when one axis is clearly stronger.
  // 1.6 means the dominant axis must be >= 60% larger than the other.
  static const double _trackpadAxisLockRatio = 1.6;
  int _trackpadSpeed = kDefaultTrackpadSpeed;
  double _trackpadSpeedInner = kDefaultTrackpadSpeed / 100.0;
  var _trackpadScrollUnsent = Offset.zero;

  // Mobile relative mouse delta accumulators (for slow/fine movements).
  double _mobileDeltaRemainderX = 0.0;
  double _mobileDeltaRemainderY = 0.0;

  var _lastScale = 1.0;

  bool _pointerMovedAfterEnter = false;
  bool _pointerInsideImage = false;

  // mouse
  final isPhysicalMouse = false.obs;
  int _lastButtons = 0;
  Offset lastMousePos = Offset.zero;
  int _lastWheelTsUs = 0;

  // Wheel acceleration thresholds.
  static const int _wheelAccelFastThresholdUs = 40000; // 40ms
  static const int _wheelAccelMediumThresholdUs = 80000; // 80ms
  static const double _wheelBurstVelocityThreshold =
      0.002; // delta units per microsecond
  // Wheel burst acceleration (empirical tuning).
  // Applies only to fast, non-smooth bursts to preserve single-step scrolling.
  // Flutter uses microseconds for dt, so velocity is in delta/us.

  // Relative mouse mode (for games/3D apps).
  final relativeMouseMode = false.obs;
  late final RelativeMouseModel _relativeMouse;
  // Callback to cancel external throttle timer when relative mouse mode is disabled.
  VoidCallback? onRelativeMouseModeDisabled;
  // Disposer for the relativeMouseMode observer (to prevent memory leaks).
  Worker? _relativeMouseModeDisposer;

  bool _queryOtherWindowCoords = false;
  Rect? _windowRect;
  List<RemoteWindowCoords> _remoteWindowCoords = [];

  late final SessionID sessionId;

  // Local gate for clipboard-assisted input flows on mobile Wayland dialogs.
  // It should not block physical keyboard events.
  bool keyboardInputAllowed = true;

  bool get keyboardPerm => parent.target!.ffiModel.keyboard;
  String get id => parent.target?.id ?? '';
  String? get peerPlatform => parent.target?.ffiModel.pi.platform;
  String get peerVersion => parent.target?.ffiModel.pi.version ?? '';
  bool get isViewOnly => parent.target!.ffiModel.viewOnly;
  bool get showMyCursor => parent.target!.ffiModel.showMyCursor;
  double get devicePixelRatio => parent.target!.canvasModel.devicePixelRatio;
  bool get isViewCamera => parent.target!.connType == ConnType.viewCamera;
  int get trackpadSpeed => _trackpadSpeed;
  bool get useEdgeScroll =>
      parent.target!.canvasModel.scrollStyle == ScrollStyle.scrolledge;

  /// Check if the connected server supports relative mouse mode.
  bool get isRelativeMouseModeSupported => _relativeMouse.isSupported;

  InputModel(this.parent) {
    initSideButtonChannel();
    sessionId = parent.target!.sessionId;
    _relativeMouse = RelativeMouseModel(
      sessionId: sessionId,
      enabled: relativeMouseMode,
      keyboardPerm: () => keyboardPerm,
      isViewCamera: () => isViewCamera,
      peerVersion: () => peerVersion,
      peerPlatform: () => peerPlatform,
      modify: (msg) => modify(msg),
      getPointerInsideImage: () => _pointerInsideImage,
      setPointerInsideImage: (inside) => _pointerInsideImage = inside,
    );
    _relativeMouse.onDisabled = () => onRelativeMouseModeDisabled?.call();

    // Sync relative mouse mode state to global state for UI components (e.g., tab bar hint).
    _relativeMouseModeDisposer = ever(relativeMouseMode, (bool value) {
      final peerId = id;
      if (peerId.isNotEmpty) {
        stateGlobal.relativeMouseModeState[peerId] = value;
      }
    });
  }

  static Map<String, dynamic> getMouseEventMove() => {
        'type': _kMouseEventMove,
        'buttons': 0,
      };

  // iOS Magic Mouse duplicate event detection.
  // When using Magic Mouse on iPad, iOS may emit both mouse and touch events
  // for the same click in certain areas (like top-left corner).
  int _lastMouseDownTimeMs = 0;
  ui.Offset _lastMouseDownPos = ui.Offset.zero;

  static Future<Rect?> fillRemoteCoordsAndGetCurFrame(
      List<RemoteWindowCoords> remoteWindowCoords) async {
    final coords =
        await rustDeskWinManager.getOtherRemoteWindowCoordsFromMain();
    final wc = WindowController.fromWindowId(kWindowId!);
    try {
      final frame = await wc.getFrame();
      for (final c in coords) {
        c.relativeOffset = Offset(
            c.windowRect.left - frame.left, c.windowRect.top - frame.top);
        remoteWindowCoords.add(c);
      }
      return frame;
    } catch (e) {
      // Unreachable code
      debugPrint("Failed to get frame of window $kWindowId, it may be hidden");
    }
    return null;
  }

  static double tryGetNearestRange(double v, double min, double max, double n) {
    if (v < min && v >= min - n) {
      v = min;
    }
    if (v > max && v <= max + n) {
      v = max;
    }
    return v;
  }

  Offset setNearestEdge(double x, double y, Rect rect) {
    double left = x - rect.left;
    double right = rect.right - 1 - x;
    double top = y - rect.top;
    double bottom = rect.bottom - 1 - y;
    if (left < right && left < top && left < bottom) {
      x = rect.left;
    }
    if (right < left && right < top && right < bottom) {
      x = rect.right - 1;
    }
    if (top < left && top < right && top < bottom) {
      y = rect.top;
    }
    if (bottom < left && bottom < right && bottom < top) {
      y = rect.bottom - 1;
    }
    return Offset(x, y);
  }

  void handlePointerEvent(String kind, String type, Offset offset) {
    double x = offset.dx;
    double y = offset.dy;
    if (_checkPeerControlProtected(x, y)) {
      return;
    }
    // Only touch events are handled for now. So we can just ignore buttons.
    // to-do: handle mouse events

    late final dynamic evtValue;
    if (type == kMouseEventTypePanUpdate) {
      evtValue = {
        'x': x.toInt(),
        'y': y.toInt(),
      };
    } else {
      final isMoveTypes = [kMouseEventTypePanStart, kMouseEventTypePanEnd];
      final pos = handlePointerDevicePos(
        kPointerEventKindTouch,
        x,
        y,
        isMoveTypes.contains(type),
        type,
      );
      if (pos == null) {
        return;
      }
      evtValue = {
        'x': pos.x.toInt(),
        'y': pos.y.toInt(),
      };
    }

    final evt = PointerEventToRust(kind, type, evtValue).toJson();
    if (isViewCamera) return;
    bind.sessionSendPointer(
        sessionId: sessionId, msg: json.encode(modify(evt)));
  }

  bool _checkPeerControlProtected(double x, double y) {
    if (isViewOnly && showMyCursor) {
      lastMousePos = ui.Offset(x, y);
      return false;
    }

    final cursorModel = parent.target!.cursorModel;
    if (cursorModel.isPeerControlProtected) {
      lastMousePos = ui.Offset(x, y);
      return true;
    }

    if (!cursorModel.gotMouseControl) {
      bool selfGetControl =
          (x - lastMousePos.dx).abs() > kMouseControlDistance ||
              (y - lastMousePos.dy).abs() > kMouseControlDistance;
      if (selfGetControl) {
        cursorModel.gotMouseControl = true;
      } else {
        lastMousePos = ui.Offset(x, y);
        return true;
      }
    }
    lastMousePos = ui.Offset(x, y);
    return false;
  }

  Map<String, dynamic>? processEventToPeer(
    Map<String, dynamic> evt,
    Offset offset, {
    bool onExit = false,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    if (isViewCamera) return null;
    double x = offset.dx;
    double y = max(0.0, offset.dy);
    if (_checkPeerControlProtected(x, y)) {
      return null;
    }

    var type = kMouseEventTypeDefault;
    var isMove = false;
    switch (evt['type']) {
      case _kMouseEventDown:
        type = kMouseEventTypeDown;
        break;
      case _kMouseEventUp:
        type = kMouseEventTypeUp;
        break;
      case _kMouseEventMove:
        _pointerMovedAfterEnter = true;
        isMove = true;
        break;
      default:
        return null;
    }
    evt['type'] = type;

    if (type == kMouseEventTypeDown && !_pointerMovedAfterEnter) {
      // Move mouse to the position of the down event first.
      lastMousePos = ui.Offset(x, y);
      refreshMousePos();
    }

    final pos = handlePointerDevicePos(
      kPointerEventKindMouse,
      x,
      y,
      isMove,
      type,
      onExit: onExit,
      buttons: evt['buttons'],
      moveCanvas: moveCanvas,
      edgeScroll: edgeScroll,
    );
    if (pos == null) {
      return null;
    }
    if (type != '') {
      evt['x'] = '0';
      evt['y'] = '0';
    } else {
      evt['x'] = '${pos.x.toInt()}';
      evt['y'] = '${pos.y.toInt()}';
    }

    final buttons = evt['buttons'];
    if (buttons is int) {
      evt['buttons'] = mouseButtonsToPeer(buttons);
    } else {
      // Log warning if buttons exists but is not an int (unexpected caller).
      // Keep empty string fallback for missing buttons to preserve move/hover behavior.
      if (buttons != null) {
        debugPrint(
            '[InputModel] processEventToPeer: unexpected buttons type: ${buttons.runtimeType}, value: $buttons');
      }
      evt['buttons'] = '';
    }
    return evt;
  }

  Map<String, dynamic>? handleMouse(
    Map<String, dynamic> evt,
    Offset offset, {
    bool onExit = false,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    final evtToPeer = processEventToPeer(evt, offset,
        onExit: onExit, moveCanvas: moveCanvas, edgeScroll: edgeScroll);
    if (evtToPeer != null) {
      bind.sessionSendMouse(
          sessionId: sessionId, msg: json.encode(modify(evtToPeer)));
    }
    return evtToPeer;
  }

  Point? handlePointerDevicePos(
    String kind,
    double x,
    double y,
    bool isMove,
    String evtType, {
    bool onExit = false,
    int buttons = kPrimaryMouseButton,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    final ffiModel = parent.target!.ffiModel;
    CanvasCoords canvas =
        CanvasCoords.fromCanvasModel(parent.target!.canvasModel);
    Rect? rect = ffiModel.rect;

    if (isMove) {
      if (_remoteWindowCoords.isNotEmpty &&
          _windowRect != null &&
          !_isInCurrentWindow(x, y)) {
        final coords =
            findRemoteCoords(x, y, _remoteWindowCoords, devicePixelRatio);
        if (coords != null) {
          isMove = false;
          canvas = coords.canvas;
          rect = coords.remoteRect;
          x -= isWindows
              ? coords.relativeOffset.dx / devicePixelRatio
              : coords.relativeOffset.dx;
          y -= isWindows
              ? coords.relativeOffset.dy / devicePixelRatio
              : coords.relativeOffset.dy;
        }
      }
    }

    y -= CanvasModel.topToEdge;
    x -= CanvasModel.leftToEdge;
    if (isMove) {
      final canvasModel = parent.target!.canvasModel;

      if (edgeScroll) {
        canvasModel.edgeScrollMouse(x, y);
      } else if (moveCanvas) {
        canvasModel.moveDesktopMouse(x, y);
      }

      canvasModel.updateLocalCursor(x, y);
    }

    return _handlePointerDevicePos(
      kind,
      x,
      y,
      isMove,
      canvas,
      rect,
      evtType,
      onExit: onExit,
      buttons: buttons,
    );
  }

  bool _isInCurrentWindow(double x, double y) {
    var w = _windowRect!.width;
    var h = _windowRect!.height;
    if (isWindows) {
      w /= devicePixelRatio;
      h /= devicePixelRatio;
    }
    return x >= 0 && y >= 0 && x <= w && y <= h;
  }

  static RemoteWindowCoords? findRemoteCoords(double x, double y,
      List<RemoteWindowCoords> remoteWindowCoords, double devicePixelRatio) {
    if (isWindows) {
      x *= devicePixelRatio;
      y *= devicePixelRatio;
    }
    for (final c in remoteWindowCoords) {
      if (x >= c.relativeOffset.dx &&
          y >= c.relativeOffset.dy &&
          x <= c.relativeOffset.dx + c.windowRect.width &&
          y <= c.relativeOffset.dy + c.windowRect.height) {
        return c;
      }
    }
    return null;
  }

  Point? _handlePointerDevicePos(
    String kind,
    double x,
    double y,
    bool moveInCanvas,
    CanvasCoords canvas,
    Rect? rect,
    String evtType, {
    bool onExit = false,
    int buttons = kPrimaryMouseButton,
  }) {
    if (rect == null) {
      return null;
    }

    final nearThr = 3;
    var nearRight = (canvas.size.width - x) < nearThr;
    var nearBottom = (canvas.size.height - y) < nearThr;
    final imageWidth = rect.width * canvas.scale;
    final imageHeight = rect.height * canvas.scale;
    if (canvas.scrollStyle != ScrollStyle.scrollauto) {
      x += imageWidth * canvas.scrollX;
      y += imageHeight * canvas.scrollY;

      // boxed size is a center widget
      if (canvas.size.width > imageWidth) {
        x -= ((canvas.size.width - imageWidth) / 2);
      }
      if (canvas.size.height > imageHeight) {
        y -= ((canvas.size.height - imageHeight) / 2);
      }
    } else {
      x -= canvas.x;
      y -= canvas.y;
    }

    x /= canvas.scale;
    y /= canvas.scale;
    if (canvas.scale > 0 && canvas.scale < 1) {
      final step = 1.0 / canvas.scale - 1;
      if (nearRight) {
        x += step;
      }
      if (nearBottom) {
        y += step;
      }
    }
    x += rect.left;
    y += rect.top;

    if (onExit) {
      final pos = setNearestEdge(x, y, rect);
      x = pos.dx;
      y = pos.dy;
    }

    return InputModel.getPointInRemoteRect(
        true, peerPlatform, kind, evtType, x, y, rect,
        buttons: buttons);
  }

  static Point<double>? getPointInRemoteRect(
      bool isLocalDesktop,
      String? peerPlatform,
      String kind,
      String evtType,
      double evtX,
      double evtY,
      Rect rect,
      {int buttons = kPrimaryMouseButton}) {
    double minX = rect.left;
    // https://github.com/rustdesk/rustdesk/issues/6678
    // For Windows, [0,maxX], [0,maxY] should be set to enable window snapping.
    double maxX = (rect.left + rect.width) -
        (peerPlatform == kPeerPlatformWindows ? 0 : 1);
    double minY = rect.top;
    double maxY = (rect.top + rect.height) -
        (peerPlatform == kPeerPlatformWindows ? 0 : 1);
    evtX = InputModel.tryGetNearestRange(evtX, minX, maxX, 5);
    evtY = InputModel.tryGetNearestRange(evtY, minY, maxY, 5);
    if (isLocalDesktop) {
      if (kind == kPointerEventKindMouse) {
        if (evtX < minX || evtY < minY || evtX > maxX || evtY > maxY) {
          // If left mouse up, no early return.
          if (!(buttons == kPrimaryMouseButton &&
              evtType == kMouseEventTypeUp)) {
            return null;
          }
        }
      }
    } else {
      bool evtXInRange = evtX >= minX && evtX <= maxX;
      bool evtYInRange = evtY >= minY && evtY <= maxY;
      if (!(evtXInRange || evtYInRange)) {
        return null;
      }
      if (evtX < minX) {
        evtX = minX;
      } else if (evtX > maxX) {
        evtX = maxX;
      }
      if (evtY < minY) {
        evtY = minY;
      } else if (evtY > maxY) {
        evtY = maxY;
      }
    }

    return Point(evtX, evtY);
  }

  /// Web only
  void listenToMouse(bool yesOrNo) {
    if (yesOrNo) {
      platformFFI.startDesktopWebListener();
    } else {
      platformFFI.stopDesktopWebListener();
    }
  }

  void onMobileBack() {
    final minBackButtonVersion = "1.3.8";
    final peerVersion =
        parent.target?.ffiModel.pi.version ?? minBackButtonVersion;
    var btn = MouseButtons.back;
    // For compatibility with old versions
    if (versionCmp(peerVersion, minBackButtonVersion) < 0) {
      btn = MouseButtons.right;
    }
    tap(btn);
  }

  void onMobileHome() => tap(MouseButtons.wheel);
  Future<void> onMobileApps() async {
    sendMouse('down', MouseButtons.wheel);
    await Future.delayed(const Duration(milliseconds: 500));
    sendMouse('up', MouseButtons.wheel);
  }

  // Simulate a key press event.
  // `usbHidUsage` is the USB HID usage code of the key.
  Future<void> tapHidKey(int usbHidUsage) async {
    newKeyboardMode(kKeyFlutterKey, usbHidUsage, true, false);
    await Future.delayed(Duration(milliseconds: 100));
    newKeyboardMode(kKeyFlutterKey, usbHidUsage, false, false);
  }

  Future<void> onMobileVolumeUp() async =>
      await tapHidKey(PhysicalKeyboardKey.audioVolumeUp.usbHidUsage & 0xFFFF);
  Future<void> onMobileVolumeDown() async =>
      await tapHidKey(PhysicalKeyboardKey.audioVolumeDown.usbHidUsage & 0xFFFF);
  Future<void> onMobilePower() async =>
      await tapHidKey(PhysicalKeyboardKey.power.usbHidUsage & 0xFFFF);
}
