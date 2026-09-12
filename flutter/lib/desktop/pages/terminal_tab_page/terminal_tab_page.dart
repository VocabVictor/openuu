import '../terminal_sessions_page.dart';
import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_copy_shortcut.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:get/get.dart';

import '../../../models/platform_model.dart';
import '../terminal_page.dart';
import '../terminal_connection_manager.dart';
import '../../widgets/material_mod_popup_menu.dart' as mod_menu;
import '../../widgets/popup_menu.dart';
import 'package:bot_toast/bot_toast.dart';
part 'sessions.dart';
part 'tabs.dart';

typedef _TerminalClipboardSource = ({
  String peerId,
  int terminalId,
  String tabKey,
});

class TerminalTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const TerminalTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<TerminalTabPage> createState() => _TerminalTabPageState(params);
}

class _TerminalTabPageState extends State<TerminalTabPage> {
  void _setState(VoidCallback fn) => setState(fn);
  DesktopTabController get tabController => Get.find<DesktopTabController>();
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

  static const IconData selectedIcon = Icons.terminal;
  static const IconData unselectedIcon = Icons.terminal_outlined;
  int _nextTerminalId = 1;
  bool _showSessions = true;
  Timer? _sessionStatusTimer;
  String _sessionSnapshot = '';

  List<TerminalSessionEntry> _sessionEntries() => tabController.state.value.tabs.map((tab) {
    final parsed = _parseTabKey(tab.key);
    final ffi = parsed == null ? null : TerminalConnectionManager.getExistingConnection(parsed.$1);
    final connected = ffi != null && !ffi.closed && ffi.terminalModels[parsed!.$2]?.terminalOpened == true;
    return TerminalSessionEntry(key: tab.key, name: tab.label, connected: connected);
  }).toList();
  // Lightweight idempotency guard for async close operations
  final Set<String> _closingTabs = {};
  // When true, all session cleanup should persist (window-level close in progress)
  bool _windowClosing = false;
  CancelFunc? _terminalClipboardNoticeCancel;
  final _terminalClipboardNotice =
      TerminalClipboardNoticeCoordinator<_TerminalClipboardSource>();

  _TerminalTabPageState(Map<String, dynamic> params) {
    Get.put(DesktopTabController(tabType: DesktopTabType.terminal));
    tabController.onSelected = (id) {
      WindowController.fromWindowId(windowId())
          .setTitle(getWindowNameWithId(id));
    };
    tabController.onRemoved = (_, id) {
      _closeTerminalClipboardNoticeForTab(id);
      onRemoveId(id);
    };
    tabController.onCloseWindow = _closeWindowFromConnection;
    final terminalId = params['terminalId'] ?? _nextTerminalId++;
    tabController.add(_createTerminalTab(
      peerId: params['id'],
      terminalId: terminalId,
      password: params['password'],
      isSharedPassword: params['isSharedPassword'],
      forceRelay: params['forceRelay'],
      connToken: params['connToken'],
    ));
  }

  void _handleTerminalClipboardWriteBlocked(
    _TerminalClipboardSource source,
    String clipboardText,
  ) {
    if (!mounted) return;
    final option = bind.mainGetLocalOption(
      key: kOptionAllowTerminalClipboardWrite,
    );
    final request = _terminalClipboardNotice.recordBlocked(
      source: source,
      text: clipboardText,
      option: option,
      canWrite: _canWriteTerminalClipboard,
    );
    if (request != null) _showTerminalClipboardNotice(request);
  }

  void _showTerminalClipboardNotice(
    TerminalClipboardNoticeRequest<_TerminalClipboardSource> request,
  ) {
    _terminalClipboardNoticeCancel = BotToast.showCustomNotification(
      duration: null,
      enableSlideOff: false,
      onlyOne: true,
      onClose: _handleTerminalClipboardNoticeClosed,
      toastBuilder: (_) => AnimatedBuilder(
        animation: _terminalClipboardNotice,
        builder: (_, __) => MaterialBanner(
          leading: const Icon(Icons.content_copy_outlined),
          content: Text(translate(kTerminalClipboardNoticeMessageKey)),
          actions: [
            TextButton(
              onPressed: _terminalClipboardNotice.canClaimAction
                  ? _handleTerminalClipboardNegativeAction
                  : null,
              child: Text(translate(request.negativeActionKey)),
            ),
            TextButton(
              onPressed: _terminalClipboardNotice.canClaimAction
                  ? _handleTerminalClipboardPositiveAction
                  : null,
              child: Text(translate(request.actionKey)),
            ),
          ],
        ),
      ),
    );
  }

  void _handleTerminalClipboardNegativeAction() {
    final request = _terminalClipboardNotice.claimCurrentAction();
    if (request == null) return;
    if (request.persistAllowed) {
      unawaited(_declineTerminalClipboardWrite());
    } else {
      _closeTerminalClipboardNotice();
    }
  }

  void _handleTerminalClipboardPositiveAction() {
    final request = _terminalClipboardNotice.claimCurrentAction();
    if (request == null) return;
    unawaited(_completeTerminalClipboardWrite(request));
  }

  void _handleTerminalClipboardNoticeClosed() {
    _terminalClipboardNoticeCancel = null;
    _terminalClipboardNotice.noticeClosed();
  }

  bool _canWriteTerminalClipboard(
    _TerminalClipboardSource source,
  ) {
    if (!_canHandleTerminalClipboardWriteRequest) return false;
    final ffi = TerminalConnectionManager.getExistingConnection(source.peerId);
    return ffi != null &&
        !ffi.closed &&
        ffi.ffiModel.permissions['clipboard'] != false &&
        tabController.state.value.tabs.any((tab) => tab.key == source.tabKey) &&
        ffi.terminalModels.containsKey(source.terminalId);
  }

  void _handleTerminalClipboardWriteSucceeded(
    _TerminalClipboardSource source,
  ) {
    final request = _terminalClipboardNotice.currentForSource(source);
    if (request == null) return;
    _closeTerminalClipboardNotice();
  }

  Future<void> _declineTerminalClipboardWrite() async {
    try {
      await bind.mainSetLocalOption(
        key: kOptionAllowTerminalClipboardWrite,
        value: kTerminalClipboardWriteDenied,
      );
    } catch (error) {
      debugPrint(
          '[TerminalTabPage] Failed to save terminal clipboard permission: $error');
      return;
    } finally {
      _terminalClipboardNotice.releaseAction();
    }
    _closeTerminalClipboardNotice();
  }

  Future<void> _completeTerminalClipboardWrite(
    TerminalClipboardNoticeRequest<_TerminalClipboardSource> request,
  ) async {
    final source = request.source;
    var completed = false;
    try {
      completed = await completeTerminalClipboardWrite(
        clipboardText: request.text,
        canWrite: () => _canWriteTerminalClipboard(source),
        writeClipboard: writeTerminalClipboard,
        persistAllowed: request.persistAllowed
            ? () => bind.mainSetLocalOption(
                  key: kOptionAllowTerminalClipboardWrite,
                  value: kTerminalClipboardWriteAllowed,
                )
            : null,
      );
    } catch (error) {
      debugPrint(
          '[TerminalTabPage] Failed to complete terminal clipboard write: $error');
    } finally {
      _terminalClipboardNotice.releaseAction();
    }
    if (!completed) return;
    _closeTerminalClipboardNotice();
  }

  void _closeTerminalClipboardNoticeForTab(String tabKey) {
    final current = _terminalClipboardNotice.current;
    if (current?.source.tabKey != tabKey) return;
    _closeTerminalClipboardNotice();
  }

  void _closeTerminalClipboardNotice() {
    if (!_terminalClipboardNotice.beginClose()) return;
    final cancel = _terminalClipboardNoticeCancel;
    if (cancel == null) {
      debugPrint('[TerminalTabPage] Clipboard notice controller is missing');
      _terminalClipboardNotice.noticeClosed();
      return;
    }
    cancel();
  }

  @override
  void initState() {
    super.initState();

    _sessionStatusTimer = Timer.periodic(const Duration(milliseconds: 500), (_) {
      if (!mounted || !_showSessions) return;
      final snapshot = _sessionEntries().map((s) => '${s.key}:${s.connected}').join('|');
      if (snapshot != _sessionSnapshot) setState(() => _sessionSnapshot = snapshot);
    });
    // Add keyboard shortcut handler
    HardwareKeyboard.instance.addHandler(_handleKeyEvent);

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      print(
          "[Remote Terminal] call ${call.method} with args ${call.arguments} from window $fromWindowId");
      if (call.method == kWindowEventNewTerminal) {
        setState(() => _showSessions = true);
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        windowOnTop(windowId());
        // Allow multiple terminals for the same connection
        final terminalId = args['terminalId'] ?? _nextTerminalId++;
        tabController.add(_createTerminalTab(
          peerId: id,
          terminalId: terminalId,
          password: args['password'],
          isSharedPassword: args['isSharedPassword'],
          forceRelay: args['forceRelay'],
          connToken: args['connToken'],
        ));
      } else if (call.method == kWindowEventRestoreTerminalSessions) {
        _restoreSessions(call.arguments);
      } else if (call.method == "onDestroy") {
        // Clean up sessions before window destruction (bounded wait)
        await _closeAllTabs();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      } else if (call.method == kWindowEventActiveSession) {
        if (tabController.state.value.tabs.isEmpty) {
          return false;
        }
        final currentTab = tabController.state.value.selectedTabInfo;
        assert(call.arguments is String,
            "Expected String arguments for kWindowEventActiveSession, got ${call.arguments.runtimeType}");
        // Use lastIndexOf to handle peerIds containing underscores
        final lastUnderscore = currentTab.key.lastIndexOf('_');
        if (lastUnderscore > 0 &&
            currentTab.key.substring(0, lastUnderscore) == call.arguments) {
          setState(() => _showSessions = true);
          windowOnTop(windowId());
          return true;
        }
        return false;
      }
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.Terminal, windowId: windowId());
    });
  }

  @override
  void dispose() {
    _sessionStatusTimer?.cancel();
    HardwareKeyboard.instance.removeHandler(_handleKeyEvent);
    _terminalClipboardNotice.clear();
    _terminalClipboardNoticeCancel?.call();
    super.dispose();
  }

  void _addNewTerminalForCurrentPeer({int? terminalId}) {
    final currentTab = tabController.state.value.selectedTabInfo;
    final parsed = _parseTabKey(currentTab.key);
    if (parsed == null) return;
    final (peerId, _) = parsed;
    _addNewTerminal(peerId, terminalId: terminalId);
  }

  @override
  Widget build(BuildContext context) {
    final terminal = Scaffold(
        backgroundColor: Theme.of(context).cardColor,
        body: DesktopTab(
          controller: tabController,
          onWindowCloseButton: handleWindowCloseButton,
          tail: _buildAddButton(),
          selectedBorderColor: MyTheme.accent,
          labelGetter: DesktopTab.tablabelGetter,
          tabMenuBuilder: (key) {
            final parsed = _parseTabKey(key);
            if (parsed == null) return Container();
            final (peerId, _) = parsed;
            return _tabMenuBuilder(peerId, () {});
          },
        ));
    final entries = _sessionEntries();
    final ready = entries.any((entry) => entry.connected);
    // Keep terminal pages mounted; authentication and PTY lifetimes remain unchanged.
    final child = Stack(children: [
      ExcludeFocus(excluding: _showSessions && ready,
        child: Offstage(offstage: _showSessions && ready, child: terminal)),
      if (_showSessions && ready) Positioned.fill(child: TerminalSessionsPage(
        device: bind.mainGetPeerOptionSync(id: widget.params['id'], key: 'alias').isNotEmpty
            ? bind.mainGetPeerOptionSync(id: widget.params['id'], key: 'alias') : widget.params['id'],
        sessions: entries,
        onCreate: _addNewTerminalForCurrentPeer,
        onOpen: (key) { tabController.jumpToByKey(key); setState(() => _showSessions = false); },
        onRemove: (key) => _closeTab(key),
        onDrag: () => WindowController.fromWindowId(windowId()).startDragging(),
        onMinimize: () => WindowController.fromWindowId(windowId()).minimize(),
        onClose: () async { if (await handleWindowCloseButton()) WindowController.fromWindowId(windowId()).close(); },
      )),
    ]);
    final tabWidget = isLinux
        ? buildVirtualWindowFrame(context, child)
        : workaroundWindowBorder(
            context,
            Container(
              decoration: BoxDecoration(
                  border: Border.all(color: MyTheme.color(context).border!)),
              child: child,
            ));
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : SubWindowDragToResizeArea(
            child: tabWidget,
            resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
            enableResizeEdges: subWindowManagerEnableResizeEdges,
            windowId: stateGlobal.windowId,
          );
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  Future<void> _closeWindowFromConnection() async {
    await _closeAllTabs();
    await WindowController.fromWindowId(windowId()).close();
  }

  int windowId() {
    return widget.params["windowId"];
  }

}
