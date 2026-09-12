import 'dart:async';
import 'dart:convert';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:xterm/xterm.dart';

import '../input_modifier_utils.dart';
import '../model.dart';
import '../platform_model.dart';
import '../rustdesk_terminal.dart';
import '../terminal_copy_shortcut.dart';
import '../terminal_mouse_handler.dart';
part 'output.dart';
part 'session.dart';

bool canConfigureTerminalClipboardPermission({
  required bool settingsDisabled,
  required bool optionFixed,
}) =>
    !settingsDisabled && !optionFixed;

bool canHandleTerminalClipboardWriteRequest({
  required String localOption,
  required bool canConfigurePermission,
}) =>
    canConfigurePermission || localOption == kTerminalClipboardWriteAllowed;

TerminalClipboardWritePermission terminalClipboardWritePermission(
  String localOption, {
  required bool remoteClipboardEnabled,
  bool canRequestConsent = true,
}) {
  if (!remoteClipboardEnabled) {
    return TerminalClipboardWritePermission.denied;
  }
  if (localOption == kTerminalClipboardWriteAllowed) {
    return TerminalClipboardWritePermission.allowed;
  }
  if (localOption == kTerminalClipboardWriteUnconfigured && canRequestConsent) {
    return TerminalClipboardWritePermission.unconfigured;
  }
  return TerminalClipboardWritePermission.denied;
}

class TerminalModel with ChangeNotifier {
  void _notify() => notifyListeners();
  final String id; // peer id
  final FFI parent;
  final int terminalId;
  late final Terminal terminal;
  late final TerminalController terminalController;

  bool _terminalOpened = false;
  bool get terminalOpened => _terminalOpened;

  bool _disposed = false;

  /// Callback to check whether Ctrl modifier lock is currently active.
  /// When active, keyboard input is mapped to control codes (e.g. 'b' → \x02).
  bool Function()? isCtrlLocked;

  /// Callback to clear Ctrl lock after a key is pressed (one-shot mode).
  void Function()? clearCtrlLock;

  /// Callback to check whether Alt modifier lock is currently active.
  bool Function()? isAltLocked;

  /// Callback to clear Alt lock after a key is pressed (one-shot mode).
  void Function()? clearAltLock;

  final _inputBuffer = <String>[];

  /// Exposes buffered input only for lifecycle regression tests.
  @visibleForTesting
  int get debugBufferedInputCount => _inputBuffer.length;

  // Buffer for output data received before terminal view has valid dimensions.
  // This prevents NaN errors when writing to terminal before layout is complete.
  final _pendingOutputChunks = <String>[];
  final _pendingOutputSuppressFlags = <bool>[];
  int _pendingOutputSize = 0;
  static const int _kMaxOutputBufferChars = 8 * 1024;
  // View ready state: true when terminal has valid dimensions, safe to write
  bool _terminalViewReady = false;
  bool _markViewReadyScheduled = false;
  bool _suppressTerminalOutput = false;
  bool _suppressNextTerminalDataOutput = false;

  void Function(int w, int h, int pw, int ph)? onResizeExternal;

  /// Called when the terminal session ends (shell exits).
  /// The listener (typically TerminalPage) can use this to auto-close the tab/page.
  VoidCallback? onClosed;

  ValueChanged<String>? onClipboardWriteBlocked;
  ValueChanged<String>? onClipboardWriteSucceeded;

  TerminalModel(this.parent, [this.terminalId = 0]) : id = parent.id {
    terminal = RustDeskTerminal(
      maxLines: 10000,
      onClipboardWrite: writeTerminalClipboard,
      clipboardWritePermission: () => terminalClipboardWritePermission(
        bind.mainGetLocalOption(key: kOptionAllowTerminalClipboardWrite),
        remoteClipboardEnabled:
            parent.ffiModel.permissions['clipboard'] != false,
        canRequestConsent: onClipboardWriteBlocked != null,
      ),
      onClipboardWriteBlocked: (text) => onClipboardWriteBlocked?.call(text),
      onClipboardWriteSucceeded: (text) =>
          onClipboardWriteSucceeded?.call(text),
    );
    terminal.mouseHandler = const WheelButtonFixMouseHandler();
    terminalController = TerminalController();

    // Setup terminal callbacks
    terminal.onOutput = (data) {
      if (_suppressTerminalOutput) return;
      _handleInput(data);
    };

    terminal.onResize = (w, h, pw, ph) async {
      // Validate all dimensions before using them
      if (w > 0 && h > 0 && pw > 0 && ph > 0) {
        debugPrint(
            '[TerminalModel] Terminal resized to ${w}x$h (pixel: ${pw}x$ph)');

        // This piece of code must be placed before the conditional check in order to initialize properly.
        onResizeExternal?.call(w, h, pw, ph);

        // Mark terminal view as ready and flush any buffered output on first valid resize.
        // Must be after onResizeExternal so the view layer has valid dimensions before flushing.
        if (!_terminalViewReady) {
          _scheduleMarkViewReady();
        }

        if (_terminalOpened) {
          // Notify remote terminal of resize
          try {
            await bind.sessionResizeTerminal(
              sessionId: parent.sessionId,
              terminalId: terminalId,
              rows: h,
              cols: w,
            );
          } catch (e) {
            debugPrint('[TerminalModel] Error resizing terminal: $e');
          }
        }
      } else {
        debugPrint(
            '[TerminalModel] Invalid terminal dimensions: ${w}x$h (pixel: ${pw}x$ph)');
      }
    };
  }

  static int getTerminalIdFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('terminal_id')) {
      final v = evt['terminal_id'];
      if (v is int) {
        // Desktop and mobile send terminal_id as an int
        return v;
      } else if (v is String) {
        // Web sends terminal_id as a string
        final parsed = int.tryParse(v);
        if (parsed != null) {
          return parsed;
        } else {
          debugPrint(
              '[TerminalModel] Failed to parse terminal_id as integer: $v. Expected a numeric string.');
          return 0;
        }
      } else {
        // Unexpected type, log and handle gracefully
        debugPrint(
            '[TerminalModel] Unexpected terminal_id type: ${v.runtimeType}, value: $v. Expected int or String.');
        return 0;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain terminal_id');
      return 0;
    }
  }

  static bool getSuccessFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('success')) {
      final v = evt['success'];
      if (v is bool) {
        // Desktop and mobile
        return v;
      } else if (v is String) {
        // Web
        return v.toLowerCase() == 'true';
      } else {
        // Unexpected type, log and handle gracefully
        debugPrint(
            '[TerminalModel] Unexpected success type: ${v.runtimeType}, value: $v. Expected bool or String.');
        return false;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain success');
      return false;
    }
  }

  static int getExitCodeFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('exit_code')) {
      final v = evt['exit_code'];
      if (v is int) {
        // Desktop and mobile send exit_code as an int
        return v;
      } else if (v is String) {
        // Web sends exit_code as a string
        final parsed = int.tryParse(v);
        if (parsed != null) {
          return parsed;
        } else {
          debugPrint(
              '[TerminalModel] Failed to parse exit_code as integer: $v. Expected a numeric string.');
          return 0;
        }
      } else {
        debugPrint(
            '[TerminalModel] Unexpected exit_code type: ${v.runtimeType}, value: $v. Expected int or String.');
        return 0;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain exit_code');
      return 0;
    }
  }

  void _handleTerminalError(Map<String, dynamic> evt) {
    final String message = evt['message'] ?? 'Unknown error';
    _writeToTerminal('\r\nTerminal error: $message\r\n');
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    terminal.onOutput = null;
    terminal.onResize = null;
    isCtrlLocked = null;
    clearCtrlLock = null;
    isAltLocked = null;
    clearAltLock = null;
    onResizeExternal = null;
    onClosed = null;
    onClipboardWriteBlocked = null;
    onClipboardWriteSucceeded = null;
    // Clear buffers to free memory
    _inputBuffer.clear();
    _pendingOutputChunks.clear();
    _pendingOutputSuppressFlags.clear();
    _pendingOutputSize = 0;
    _markViewReadyScheduled = false;
    _suppressNextTerminalDataOutput = false;
    // Terminal cleanup is handled server-side when service closes
    super.dispose();
  }
}
