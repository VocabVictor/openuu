import 'dart:async';
import 'dart:math';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/terminal_copy_shortcut.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:flutter_hbb/models/terminal_mouse_handler.dart';
import 'package:flutter_hbb/mobile/terminal_keyboard_utils.dart';
import 'package:flutter_hbb/native/unsupported_web.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:xterm/xterm.dart';
import '../../../desktop/pages/terminal_connection_manager.dart';
import '../../../consts.dart';
part 'keyboard.dart';
part 'body.dart';
part 'clipboard.dart';

const _terminalBackgroundOpacity = 0.7;

Widget _buildTerminalViewForPlatform({
  required bool reportMouseInput,
  required bool reportTouchInput,
  required Terminal terminal,
  required TerminalController controller,
  required TerminalStyle textStyle,
  required EdgeInsets padding,
  required bool deleteDetection,
  required Map<ShortcutActivator, Intent>? shortcuts,
  required FocusOnKeyEventCallback onKeyEvent,
  required void Function(TapDownDetails, CellOffset) onSecondaryTapDown,
}) {
  if (reportMouseInput || reportTouchInput) {
    return TerminalMouseInteraction(
      terminal,
      controller: controller,
      autofocus: true,
      textStyle: textStyle,
      deleteDetection: deleteDetection,
      reportTouchInput: reportTouchInput,
      shortcuts: shortcuts,
      onKeyEvent: onKeyEvent,
      backgroundOpacity: _terminalBackgroundOpacity,
      padding: padding,
      onSecondaryTapDown: onSecondaryTapDown,
    );
  }
  return TerminalView(
    terminal,
    controller: controller,
    autofocus: true,
    textStyle: textStyle,
    deleteDetection: deleteDetection,
    shortcuts: shortcuts,
    onKeyEvent: onKeyEvent,
    backgroundOpacity: _terminalBackgroundOpacity,
    padding: padding,
    onSecondaryTapDown: onSecondaryTapDown,
  );
}

class TerminalPage extends StatefulWidget {
  const TerminalPage({
    Key? key,
    required this.id,
    required this.password,
    required this.isSharedPassword,
    this.forceRelay,
    this.connToken,
  }) : super(key: key);
  final String id;
  final String? password;
  final bool? forceRelay;
  final bool? isSharedPassword;
  final String? connToken;
  final terminalId = 0;

  @override
  State<TerminalPage> createState() => _TerminalPageState();
}

class _TerminalPageState extends State<TerminalPage>
    with AutomaticKeepAliveClientMixin, WidgetsBindingObserver {
  void _setState(VoidCallback fn) => setState(fn);
  bool get _canConfigureTerminalClipboardPermission =>
      canConfigureTerminalClipboardPermission(
        settingsDisabled: bind.isDisableSettings(),
        optionFixed: isOptionFixed(kOptionAllowTerminalClipboardWrite),
      );
  bool get _canHandleTerminalClipboardWriteRequest =>
      canHandleTerminalClipboardWriteRequest(
        localOption: bind.mainGetLocalOption(
          key: kOptionAllowTerminalClipboardWrite,
        ),
        canConfigurePermission: _canConfigureTerminalClipboardPermission,
      );

  late FFI _ffi;
  late TerminalModel _terminalModel;
  double? _cellHeight;
  double _sysKeyboardHeight = 0;
  Timer? _keyboardDebounce;
  final GlobalKey _keyboardKey = GlobalKey();
  double _keyboardHeight = 0;
  late bool _showTerminalExtraKeys;
  // Ctrl lock state for virtual keyboard: active key presses are mapped to control codes
  bool _ctrlLocked = false;
  bool _altLocked = false;
  // Row3 expand/collapse state for compact keyboard layout
  bool _row3Expanded = false;
  // For iOS edge swipe gesture
  double _swipeStartX = 0;
  double _swipeCurrentX = 0;
  ScaffoldFeatureController<MaterialBanner, MaterialBannerClosedReason>?
      _terminalClipboardNoticeController;
  final _terminalClipboardNotice = TerminalClipboardNoticeCoordinator<int>();

  // For web only.
  // 'monospace' does not work on web, use Google Fonts, `??` is only for null safety.
  final String _robotoMonoFontFamily = isWeb
      ? (GoogleFonts.robotoMono().fontFamily ?? 'monospace')
      : 'monospace';

  SessionID get sessionId => _ffi.sessionId;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    if (isWeb) {
      loadLocalTerminalFontIfNeeded();
    }

    debugPrint(
        '[TerminalPage] Initializing terminal ${widget.terminalId} for peer ${widget.id}');

    // Use shared FFI instance from connection manager
    _ffi = TerminalConnectionManager.getConnection(
      peerId: widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
      connToken: widget.connToken,
    );

    // Create terminal model with specific terminal ID
    _terminalModel = TerminalModel(_ffi, widget.terminalId);
    if (_canHandleTerminalClipboardWriteRequest) {
      _terminalModel.onClipboardWriteBlocked =
          _handleTerminalClipboardWriteBlocked;
      _terminalModel.onClipboardWriteSucceeded =
          _handleTerminalClipboardWriteSucceeded;
    }
    debugPrint(
        '[TerminalPage] Terminal model created for terminal ${widget.terminalId}');

    _terminalModel.onResizeExternal = (w, h, pw, ph) {
      _cellHeight = ph * 1.0;
    };

    // Register this terminal model with FFI for event routing
    _ffi.registerTerminalModel(widget.terminalId, _terminalModel);

    // Auto-close connection when shell exits
    _terminalModel.onClosed = () {
      if (mounted) {
        closeConnection(id: widget.id);
      }
    };

    // Web desktop users have full hardware keyboard access, so the on-screen
    // terminal extra keys bar is unnecessary and disabled.
    _showTerminalExtraKeys = !isWebDesktop &&
        mainGetLocalBoolOptionSync(kOptionEnableShowTerminalExtraKeys);
    _terminalModel.isCtrlLocked = () => _ctrlLocked;
    _terminalModel.clearCtrlLock = () {
      if (_ctrlLocked) setState(() => _ctrlLocked = false);
    };
    _terminalModel.isAltLocked = () => _altLocked;
    _terminalModel.clearAltLock = () {
      if (_altLocked) setState(() => _altLocked = false);
    };
    // Load Row3 expand/collapse state from persistent storage. The raw option
    // read keeps Row3 collapsed when no value has been saved yet.
    _row3Expanded =
        bind.mainGetLocalOption(key: kOptionShowTerminalCtrlKeys) == 'Y';
    // Initialize terminal connection
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);

      if (_showTerminalExtraKeys) {
        _updateKeyboardHeight();
      }
    });
    _ffi.ffiModel.updateEventListener(_ffi.sessionId, widget.id);
  }

  @override
  void dispose() {
    // Unregister terminal model from FFI
    _ffi.unregisterTerminalModel(widget.terminalId);
    _terminalModel.dispose();
    _keyboardDebounce?.cancel();
    _terminalClipboardNotice.clear();
    _terminalClipboardNoticeController?.close();
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
    TerminalConnectionManager.releaseConnection(widget.id);
  }

  @override
  void didChangeMetrics() {
    super.didChangeMetrics();

    _keyboardDebounce?.cancel();
    _keyboardDebounce = Timer(const Duration(milliseconds: 20), () {
      final bottomInset = MediaQuery.of(context).viewInsets.bottom;
      setState(() {
        _sysKeyboardHeight = bottomInset;
      });
    });
  }

  void _updateKeyboardHeight() {
    if (_keyboardKey.currentContext != null) {
      final renderBox = _keyboardKey.currentContext!.findRenderObject() as RenderBox;
      _keyboardHeight = renderBox.size.height;
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return WillPopScope(
      onWillPop: () async {
        clientClose(sessionId, _ffi);
        return false; // Prevent default back behavior
      },
      child: buildBody(),
    );
  }

  @override
  bool get wantKeepAlive => true;
}
