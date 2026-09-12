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

  void onPointHoverImage(PointerHoverEvent e) {
    _stopFling = true;
    if (isViewOnly && !showMyCursor) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;

    // May fix https://github.com/rustdesk/rustdesk/issues/13009
    if (isIOS && e.synthesized && e.position == Offset.zero && e.buttons == 0) {
      // iOS may emit a synthesized hover event at (0,0) when the mouse is disconnected.
      // Ignore this event to prevent cursor jumping.
      debugPrint('Ignored synthesized hover at (0,0) on iOS');
      return;
    }

    // Only update pointer region when relative mouse mode is enabled.
    // This avoids unnecessary tracking when not in relative mode.
    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (!isPhysicalMouse.value) {
      isPhysicalMouse.value = true;
    }
    if (isPhysicalMouse.value) {
      if (!_relativeMouse.handleRelativeMouseMove(e.localPosition)) {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventMove), canvasPosition,
            edgeScroll: useEdgeScroll);
      }
    }
  }

  void onPointerPanZoomStart(PointerPanZoomStartEvent e) {
    _lastScale = 1.0;
    _stopFling = true;
    if (isViewOnly) return;
    if (isViewCamera) return;
    if (peerPlatform == kPeerPlatformAndroid) {
      handlePointerEvent('touch', kMouseEventTypePanStart, e.position);
    }
  }

  // https://docs.flutter.dev/release/breaking-changes/trackpad-gestures
  void onPointerPanZoomUpdate(PointerPanZoomUpdateEvent e) {
    if (isViewOnly) return;
    if (isViewCamera) return;
    if (peerPlatform != kPeerPlatformAndroid) {
      final scale = ((e.scale - _lastScale) * 1000).toInt();
      _lastScale = e.scale;

      if (scale != 0) {
        bind.sessionSendPointer(
            sessionId: sessionId,
            msg: json.encode(
                PointerEventToRust(kPointerEventKindTouch, 'scale', scale)
                    .toJson()));
        return;
      }
    }

    var delta = e.panDelta * _trackpadSpeedInner;
    if (isMacOS && peerPlatform == kPeerPlatformWindows) {
      delta *= _trackpadAdjustMacToWin;
    }
    delta = _filterTrackpadDeltaAxis(delta);
    _trackpadLastDelta = delta;

    var x = delta.dx.toInt();
    var y = delta.dy.toInt();
    if (peerPlatform == kPeerPlatformLinux) {
      _trackpadScrollUnsent += (delta * _trackpadAdjustPeerLinux);
      x = _trackpadScrollUnsent.dx.truncate();
      y = _trackpadScrollUnsent.dy.truncate();
      _trackpadScrollUnsent -= Offset(x.toDouble(), y.toDouble());
    } else {
      if (x == 0 && y == 0) {
        final thr = 0.1;
        if (delta.dx.abs() > delta.dy.abs()) {
          x = delta.dx > thr ? 1 : (delta.dx < -thr ? -1 : 0);
        } else {
          y = delta.dy > thr ? 1 : (delta.dy < -thr ? -1 : 0);
        }
      }
    }
    if (x != 0 || y != 0) {
      if (peerPlatform == kPeerPlatformAndroid) {
        handlePointerEvent('touch', kMouseEventTypePanUpdate,
            Offset(x.toDouble(), y.toDouble()));
      } else {
        if (isViewCamera) return;
        bind.sessionSendMouse(
            sessionId: sessionId,
            msg: '{"type": "trackpad", "x": "$x", "y": "$y"}');
      }
    }
  }

  Offset _filterTrackpadDeltaAxis(Offset delta) {
    final absDx = delta.dx.abs();
    final absDy = delta.dy.abs();
    // Keep diagonal intent when movement is tiny on both axes.
    if (absDx < _trackpadAxisNoiseThreshold &&
        absDy < _trackpadAxisNoiseThreshold) {
      return delta;
    }
    // Dominant-axis lock to reduce accidental cross-axis scrolling noise.
    if (absDy >= absDx * _trackpadAxisLockRatio) {
      return Offset(0, delta.dy);
    }
    if (absDx >= absDy * _trackpadAxisLockRatio) {
      return Offset(delta.dx, 0);
    }
    return delta;
  }

  void _scheduleFling(double x, double y, int delay) {
    if (isViewCamera) return;
    if ((x == 0 && y == 0) || _stopFling) {
      _fling = false;
      return;
    }

    _flingTimer = Timer(Duration(milliseconds: delay), () {
      if (_stopFling) {
        _fling = false;
        return;
      }

      final d = 0.97;
      x *= d;
      y *= d;

      // Try set delta (x,y) and delay.
      var dx = x.toInt();
      var dy = y.toInt();
      if (parent.target?.ffiModel.pi.platform == kPeerPlatformLinux) {
        dx = (x * _trackpadAdjustPeerLinux).toInt();
        dy = (y * _trackpadAdjustPeerLinux).toInt();
      }

      var delay = _flingBaseDelay;

      if (dx == 0 && dy == 0) {
        _fling = false;
        return;
      }

      bind.sessionSendMouse(
          sessionId: sessionId,
          msg: '{"type": "trackpad", "x": "$dx", "y": "$dy"}');
      _scheduleFling(x, y, delay);
    });
  }

  void waitLastFlingDone() {
    if (_fling) {
      _stopFling = true;
    }
    for (var i = 0; i < 5; i++) {
      if (!_fling) {
        break;
      }
      sleep(Duration(milliseconds: 10));
    }
    _flingTimer?.cancel();
  }

  void onPointerPanZoomEnd(PointerPanZoomEndEvent e) {
    if (isViewCamera) return;
    if (peerPlatform == kPeerPlatformAndroid) {
      handlePointerEvent('touch', kMouseEventTypePanEnd, e.position);
      return;
    }

    bind.sessionSendPointer(
        sessionId: sessionId,
        msg: json.encode(
            PointerEventToRust(kPointerEventKindTouch, 'scale', 0).toJson()));

    waitLastFlingDone();
    _stopFling = false;

    // 2.0 is an experience value
    double minFlingValue = 2.0 * _trackpadSpeedInner;
    if (isMacOS && peerPlatform == kPeerPlatformWindows) {
      minFlingValue *= _trackpadAdjustMacToWin;
    }
    if (_trackpadLastDelta.dx.abs() > minFlingValue ||
        _trackpadLastDelta.dy.abs() > minFlingValue) {
      _fling = true;
      _scheduleFling(
          _trackpadLastDelta.dx, _trackpadLastDelta.dy, _flingBaseDelay);
    }
    _trackpadLastDelta = Offset.zero;
  }

  // iOS Magic Mouse duplicate event detection.
  // When using Magic Mouse on iPad, iOS may emit both mouse and touch events
  // for the same click in certain areas (like top-left corner).
  int _lastMouseDownTimeMs = 0;
  ui.Offset _lastMouseDownPos = ui.Offset.zero;

  /// Check if a touch tap event should be ignored because it's a duplicate
  /// of a recent mouse event (iOS Magic Mouse issue).
  bool shouldIgnoreTouchTap(ui.Offset pos) {
    if (!isIOS) return false;
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    final dt = nowMs - _lastMouseDownTimeMs;
    final distance = (_lastMouseDownPos - pos).distance;
    // If touch tap is within 2000ms and 80px of the last mouse down,
    // it's likely a duplicate event from the same Magic Mouse click.
    if (dt >= 0 && dt < 2000 && distance < 80.0) {
      debugPrint("shouldIgnoreTouchTap: IGNORED (dt=$dt, dist=$distance)");
      return true;
    }
    return false;
  }

  /// iOS may emit a synthesized touch event after a real mouse click.
  /// This helper ignores touch-down events that arrive shortly after a mouse down,
  /// even when the position is far (e.g., near the top edge).
  bool _shouldIgnoreTouchAfterMouse(int nowMs) {
    if (!isIOS) return false;
    const int kTouchAfterMouseWindowMs = 700;
    final dt = nowMs - _lastMouseDownTimeMs;
    return dt >= 0 && dt < kTouchAfterMouseWindowMs;
  }

  void onPointDownImage(PointerDownEvent e) {
    debugPrint("onPointDownImage ${e.kind}");
    _stopFling = true;
    if (isDesktop) _queryOtherWindowCoords = true;
    _remoteWindowCoords = [];
    _windowRect = null;
    if (isViewOnly && !showMyCursor) return;
    if (isViewCamera) return;

    // Track mouse down events for duplicate detection on iOS.
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    if (e.kind == ui.PointerDeviceKind.mouse) {
      if (!isPhysicalMouse.value) {
        isPhysicalMouse.value = true;
      }
      _lastMouseDownTimeMs = nowMs;
      _lastMouseDownPos = e.position;
    }

    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (e.kind != ui.PointerDeviceKind.mouse) {
      // Ignore duplicate touch events that follow a recent mouse click (iOS Magic Mouse issue).
      if (isPhysicalMouse.value && _shouldIgnoreTouchAfterMouse(nowMs)) {
        return;
      }
      if (isPhysicalMouse.value) {
        isPhysicalMouse.value = false;
      }
    }
    if (isPhysicalMouse.value) {
      // In relative mouse mode, send button events without position.
      // Use _relativeMouse.enabled.value consistently with the guard above.
      if (_relativeMouse.enabled.value) {
        _relativeMouse
            .sendRelativeMouseButton(_getMouseEvent(e, _kMouseEventDown));
      } else {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventDown), canvasPosition);
      }
    }
  }

  void onPointUpImage(PointerUpEvent e) {
    if (isDesktop) _queryOtherWindowCoords = false;
    if (isViewOnly && !showMyCursor) return;
    if (isViewCamera) return;

    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (isPhysicalMouse.value) {
      // In relative mouse mode, send button events without position.
      // Use _relativeMouse.enabled.value consistently with the guard above.
      if (_relativeMouse.enabled.value) {
        _relativeMouse
            .sendRelativeMouseButton(_getMouseEvent(e, _kMouseEventUp));
      } else {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventUp), canvasPosition);
      }
    }
  }

  void onPointMoveImage(PointerMoveEvent e) {
    if (isViewOnly && !showMyCursor) return;
    if (isViewCamera) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;

    if (_relativeMouse.enabled.value) {
      _relativeMouse.updatePointerRegionTopLeftGlobal(e);
    }

    if (_queryOtherWindowCoords) {
      Future.delayed(Duration.zero, () async {
        _windowRect = await fillRemoteCoordsAndGetCurFrame(_remoteWindowCoords);
      });
      _queryOtherWindowCoords = false;
    }
    if (isPhysicalMouse.value) {
      if (!_relativeMouse.handleRelativeMouseMove(e.localPosition)) {
        final canvasPosition = _pointerPositionForRemoteCanvas(e);
        handleMouse(_getMouseEvent(e, _kMouseEventMove), canvasPosition,
            edgeScroll: useEdgeScroll);
      }
    }
  }

  /// Convert pointer coordinates into the visible remote canvas space.
  ///
  /// On mobile, the remote page body is wrapped in `SafeArea`, but the pointer
  /// listener that feeds these events sits outside that subtree. As a result,
  /// `event.localPosition` still includes the top/left safe-area inset.
  ///
  /// When the keyboard-visible path shows `KeyHelpTools`, the remote canvas is
  /// also shifted downward by `CanvasModel.getAdjustY()`. The downstream mouse
  /// mapping logic expects coordinates relative to the visible canvas area, so
  /// we subtract both the mobile safe-area padding and the current canvas
  /// adjustment before passing the position into mouse mapping.
  ///
  /// Desktop and web desktop continue to use the global position directly
  /// because their pointer mapping is window-based.
  Offset _pointerPositionForRemoteCanvas(PointerEvent event) {
    if (isDesktop || isWebDesktop) {
      return event.position;
    }
    final mediaData = MediaQueryData.fromView(
        WidgetsBinding.instance.platformDispatcher.views.first);
    final adjustY = parent.target?.canvasModel.getAdjustY() ?? 0.0;
    return Offset(
      event.localPosition.dx - mediaData.padding.left,
      event.localPosition.dy - mediaData.padding.top - adjustY,
    );
  }

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

  /// Handle scroll/wheel events.
  /// Note: Scroll events intentionally use absolute positioning even in relative mouse mode.
  /// This is because scroll events don't need relative positioning - they represent
  /// scroll deltas that are independent of cursor position. Games and 3D applications
  /// handle scroll events the same way regardless of mouse mode.
  void onPointerSignalImage(PointerSignalEvent e) {
    if (isViewOnly) return;
    if (isViewCamera) return;
    if (e is PointerScrollEvent) {
      final rawDx = e.scrollDelta.dx;
      final rawDy = e.scrollDelta.dy;
      final dominantDelta = rawDx.abs() > rawDy.abs() ? rawDx.abs() : rawDy.abs();
      final isSmooth = dominantDelta < 1;
      final nowUs = DateTime.now().microsecondsSinceEpoch;
      final dtUs = _lastWheelTsUs == 0 ? 0 : nowUs - _lastWheelTsUs;
      _lastWheelTsUs = nowUs;
      int accel = 1;
      if (!isSmooth &&
          dtUs > 0 &&
          dtUs <= _wheelAccelMediumThresholdUs &&
          (isWindows || isLinux) &&
          peerPlatform == kPeerPlatformMacOS) {
        final velocity = dominantDelta / dtUs;
        if (velocity >= _wheelBurstVelocityThreshold) {
          if (dtUs < _wheelAccelFastThresholdUs) {
            accel = 3;
          } else {
            accel = 2;
          }
        }
      }
      var dx = rawDx.toInt();
      var dy = rawDy.toInt();
      if (rawDx.abs() > rawDy.abs()) {
        dy = 0;
      } else {
        dx = 0;
      }
      if (dx > 0) {
        dx = -accel;
      } else if (dx < 0) {
        dx = accel;
      }
      if (dy > 0) {
        dy = -accel;
      } else if (dy < 0) {
        dy = accel;
      }
      bind.sessionSendMouse(
          sessionId: sessionId,
          msg: '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }
  }

  void refreshMousePos() => handleMouse({
        'buttons': 0,
        'type': _kMouseEventMove,
      }, lastMousePos, edgeScroll: useEdgeScroll);

  void tryMoveEdgeOnExit(Offset pos) => handleMouse(
        {
          'buttons': 0,
          'type': _kMouseEventMove,
        },
        pos,
        onExit: true,
      );

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
