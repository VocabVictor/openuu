import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/relative_mouse_accumulator.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../consts.dart';
import '../platform_model.dart';
part 'cursor_clip.dart';
part 'pointer_lock.dart';
part 'move.dart';
part 'mode.dart';
part 'events.dart';
part 'native.dart';

class RelativeMouseModel {
  final SessionID sessionId;
  final RxBool enabled;

  final bool Function() keyboardPerm;
  final bool Function() isViewCamera;
  final String Function() peerVersion;
  final String? Function() peerPlatform;

  final Map<String, dynamic> Function(Map<String, dynamic> msg) modify;

  final bool Function() getPointerInsideImage;
  final void Function(bool inside) setPointerInsideImage;

  RelativeMouseModel({
    required this.sessionId,
    required this.enabled,
    required this.keyboardPerm,
    required this.isViewCamera,
    required this.peerVersion,
    required this.peerPlatform,
    required this.modify,
    required this.getPointerInsideImage,
    required this.setPointerInsideImage,
  });

  final RelativeMouseAccumulator _accumulator = RelativeMouseAccumulator();

  // Native relative mouse mode support (macOS only)
  // Uses CGAssociateMouseAndMouseCursorPosition to lock cursor and NSEvent monitor for raw delta.
  static MethodChannel? _hostChannel;
  // The currently active model receiving native mouse delta events.
  // Note: Race condition between multiple sessions is not a concern here because
  // when relative mouse mode is active, the cursor is locked and the user cannot
  // switch to another session window. The user must first exit relative mouse mode
  // (via Cmd+G on macOS or Ctrl+Alt on Windows/Linux) before they can interact
  // with a different session.
  static RelativeMouseModel? _activeNativeModel;
  static bool _hostChannelInitialized = false;

  /// Initialize the host channel for native relative mouse mode.
  /// This should be called once when the app starts on macOS.
  static void initHostChannel() {
    if (!isMacOS) return;
    if (_hostChannelInitialized) return;
    _hostChannelInitialized = true;

    _hostChannel = const MethodChannel('org.rustdesk.rustdesk/host');
    _hostChannel!.setMethodCallHandler((call) async {
      if (call.method == 'onMouseDelta') {
        final args = call.arguments as Map<dynamic, dynamic>;
        final dx = args['dx'] as int;
        final dy = args['dy'] as int;
        _activeNativeModel?._onNativeMouseDelta(dx, dy);
      }
      return null;
    });
  }

  // Whether native relative mouse mode is currently active for this model
  bool get _isNativeRelativeMouseModeActive =>
      isMacOS && _activeNativeModel == this;

  // Pointer lock center in LOCAL widget coordinates (for delta calculation)
  Offset? _pointerLockCenterLocal;
  // Pointer lock center in SCREEN coordinates (for OS cursor re-centering)
  Offset? _pointerLockCenterScreen;
  // Pointer region top-left in Flutter view coordinates.
  // Computed from PointerEvent.position - PointerEvent.localPosition.
  Offset? _pointerRegionTopLeftGlobal;
  // Last pointer position in LOCAL widget coordinates (fallback when center is not ready).
  Offset? _lastPointerLocalPos;

  // Track whether we currently have an OS-level cursor clip active (Windows only).
  // TODO(accuracy): Revisit window/client/border clipping math if users report misaligned
  // clipping on custom or maximized window decorations. Consider using platform APIs
  // (e.g. GetClientRect on Windows) instead of Flutter's window coordinates.
  bool _cursorClipApplied = false;

  // Track whether a recenter operation is in progress to prevent overlapping calls.
  bool _recenterInProgress = false;

  // Request token for async enable operation to prevent stale callbacks.
  // Incremented on each enable attempt, callbacks check if token still matches.
  int _enableRequestId = 0;

  // Throttle buffer for batching mouse move messages (reduces network flooding).
  int _pendingDeltaX = 0;
  int _pendingDeltaY = 0;
  Timer? _throttleTimer;
  static const Duration _throttleInterval = Duration(milliseconds: 16);

  // Size of the remote image widget (for center calculation)
  Size? _imageWidgetSize;

  // Debounce timestamp for relative mouse mode toggle to prevent race conditions
  // between Rust rdev grab loop and Flutter keyboard handling.
  DateTime? _lastToggle;

  // Track key down state for exit shortcut.
  // macOS: Cmd+G - track G key
  // Windows/Linux: Ctrl+Alt - track whichever modifier was pressed last
  // When key down is blocked (shortcut triggered), we also need to block
  // the corresponding key up to avoid orphan key up events being sent to remote.
  bool _exitShortcutKeyDown = false;

  // Callback to cancel external throttle timer when relative mouse mode is disabled.
  VoidCallback? onDisabled;

  // Flag to skip the first mouse move event after recenter (it's the recenter itself).
  bool _skipNextMouseMove = false;

  // Edge threshold parameters for recenter detection.
  // Threshold is calculated as: min(maxThreshold, min(width, height) * fraction)
  static const double _edgeThresholdFraction = 0.1; // 10% of smaller dimension
  static const double _edgeThresholdMax =
      100.0; // Maximum threshold in logical pixels
  static const double _edgeThresholdMin =
      20.0; // Minimum threshold for very small widgets

  // Linux-specific edge threshold parameters (more aggressive to prevent cursor escape).
  // On Linux, we don't have clip_cursor capability, so we need to recenter earlier
  // to prevent the cursor from escaping the window when moving fast.
  static const double _edgeThresholdFractionLinux =
      0.25; // 25% of smaller dimension
  static const double _edgeThresholdMaxLinux =
      200.0; // Larger maximum threshold for Linux
  static const double _edgeThresholdMinLinux =
      50.0; // Larger minimum threshold for Linux

  static String _mouseEventTypeToPeer(String type) {
    switch (type) {
      case 'mousedown':
        return kMouseEventTypeDown;
      case 'mouseup':
        return kMouseEventTypeUp;
      default:
        return '';
    }
  }

  /// Retry parameters for cursor re-centering.
  static const int _recenterMaxRetries = 3;
  static const Duration _recenterRetryDelay = Duration(milliseconds: 100);

  void _disableWithCleanup() {
    _performCleanupCore();
    enabled.value = false;
    onDisabled?.call();
  }

  bool _disposed = false;

  void dispose() {
    if (_disposed) return;
    _disposed = true;

    _performCleanupCore();
    _imageWidgetSize = null;
    _lastToggle = null;
    // Set enabled to false BEFORE calling onDisabled, consistent with _disableWithCleanup().
    enabled.value = false;
    // Trigger callback before clearing it, so external cleanup can run.
    onDisabled?.call();
    onDisabled = null;
  }
}
