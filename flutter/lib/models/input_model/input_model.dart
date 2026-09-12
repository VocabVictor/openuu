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
part 'pointer_pos.dart';
part 'pointer.dart';

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

}
