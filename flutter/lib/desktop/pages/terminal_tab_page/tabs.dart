part of 'terminal_tab_page.dart';

extension _TerminalTabTabs on _TerminalTabPageState {
  /// Unified tab close handler for all close paths (button, shortcut, programmatic).
  /// Shows audit dialog, cleans up session if not persistent, then removes the UI tab.
  Future<void> _closeTab(String tabKey) async {
    // Idempotency guard: skip if already closing this tab
    if (_closingTabs.contains(tabKey)) return;
    _closingTabs.add(tabKey);

    try {
      // Snapshot peerTabCount BEFORE any await to avoid race with concurrent
      // _closeAllTabs clearing tabController (which would make the live count
      // drop to 0 and incorrectly trigger session persistence).
      // Note: the snapshot may become stale if other individual tabs are closed
      // during the audit dialog, but this is an acceptable trade-off.
      int? snapshotPeerTabCount;
      final parsed = _parseTabKey(tabKey);
      if (parsed != null) {
        final (peerId, _) = parsed;
        snapshotPeerTabCount = tabController.state.value.tabs.where((t) {
          final p = _parseTabKey(t.key);
          return p != null && p.$1 == peerId;
        }).length;
      }

      if (await desktopTryShowTabAuditDialogCloseCancelled(
        id: tabKey,
        tabController: tabController,
      )) {
        return;
      }

      // Close terminal session if not in persistent mode.
      // Wrapped separately so session cleanup failure never blocks UI tab removal.
      try {
        await _closeTerminalSessionIfNeeded(tabKey,
            peerTabCount: snapshotPeerTabCount);
      } catch (e) {
        debugPrint('[TerminalTabPage] Session cleanup failed for $tabKey: $e');
      }
      // Always close the tab from UI, regardless of session cleanup result
      tabController.closeBy(tabKey);
    } catch (e) {
      debugPrint('[TerminalTabPage] Error closing tab $tabKey: $e');
    } finally {
      _closingTabs.remove(tabKey);
    }
  }

  /// Close all tabs with session cleanup.
  /// Used for window-level close operations (onDestroy, handleWindowCloseButton).
  /// UI tabs are removed immediately; session cleanup runs in parallel with a
  /// bounded timeout so window close is not blocked indefinitely.
  Future<void> _closeAllTabs() async {
    _windowClosing = true;
    final tabKeys = tabController.state.value.tabs.map((t) => t.key).toList();
    // Remove all UI tabs immediately (same instant behavior as the old tabController.clear())
    // Keep the cleanup target lookup below synchronous before its first await:
    // it relies on the current frame still retaining each TerminalPage's FFI/model.
    _terminalClipboardNotice.clear();
    _terminalClipboardNoticeCancel?.call();
    tabController.clear();
    // Run session cleanup in parallel with bounded timeout (closeTerminal() has internal 3s timeout).
    // Skip tabs already being closed by a concurrent _closeTab() to avoid duplicate FFI calls.
    final futures = tabKeys
        .where((tabKey) => !_closingTabs.contains(tabKey))
        .map((tabKey) async {
      try {
        await _closeTerminalSessionIfNeeded(tabKey, persistAll: true);
      } catch (e) {
        debugPrint('[TerminalTabPage] Session cleanup failed for $tabKey: $e');
      }
    }).toList();
    if (futures.isNotEmpty) {
      await Future.wait(futures).timeout(
        const Duration(seconds: 4),
        onTimeout: () {
          debugPrint(
              '[TerminalTabPage] Session cleanup timed out for batch close');
          return [];
        },
      );
    }
  }

  /// Close the terminal session on server side based on persistent mode.
  ///
  /// [persistAll] controls behavior when persistent mode is enabled:
  /// - `true` (window close): persist all sessions, don't close any.
  /// - `false` (tab close): only persist the last session for the peer,
  ///   close others so only the most recent disconnected session survives.
  ///
  /// Note: if [_windowClosing] is true, persistAll is forced to true so that
  /// in-flight _closeTab() calls don't accidentally close sessions that the
  /// window-close flow intends to preserve.
  Future<void> _closeTerminalSessionIfNeeded(String tabKey,
      {bool persistAll = false, int? peerTabCount}) async {
    // If window close is in progress, override to persist all sessions
    // even if this call originated from an individual tab close.
    if (_windowClosing) {
      persistAll = true;
    }
    final parsed = _parseTabKey(tabKey);
    if (parsed == null) return;
    final (peerId, terminalId) = parsed;

    final ffi = TerminalConnectionManager.getExistingConnection(peerId);
    if (ffi == null) return;

    final isPersistent = bind.sessionGetToggleOptionSync(
      sessionId: ffi.sessionId,
      arg: kOptionTerminalPersistent,
    );

    if (isPersistent) {
      if (persistAll) {
        // Window close: persist all sessions
        return;
      }
      // Tab close: only persist if this is the last tab for this peer.
      // Use the snapshot value if provided (avoids race with concurrent tab removal).
      final effectivePeerTabCount = peerTabCount ??
          tabController.state.value.tabs.where((t) {
            final p = _parseTabKey(t.key);
            return p != null && p.$1 == peerId;
          }).length;
      if (effectivePeerTabCount <= 1) {
        // Last tab for this peer — persist the session
        return;
      }
      // Not the last tab — fall through to close the session
    }

    final terminalModel = ffi.terminalModels[terminalId];
    if (terminalModel != null) {
      // closeTerminal() has internal 3s timeout, no need for external timeout
      await terminalModel.closeTerminal();
    }
  }

  /// Parse tabKey (format: "peerId_terminalId") into its components.
  /// Note: peerId may contain underscores, so we use lastIndexOf('_').
  /// Returns null if tabKey format is invalid.
  (String peerId, int terminalId)? _parseTabKey(String tabKey) {
    final lastUnderscore = tabKey.lastIndexOf('_');
    if (lastUnderscore <= 0) {
      debugPrint('[TerminalTabPage] Invalid tabKey format: $tabKey');
      return null;
    }
    final terminalIdStr = tabKey.substring(lastUnderscore + 1);
    final terminalId = int.tryParse(terminalIdStr);
    if (terminalId == null) {
      debugPrint('[TerminalTabPage] Invalid terminalId in tabKey: $tabKey');
      return null;
    }
    final peerId = tabKey.substring(0, lastUnderscore);
    return (peerId, terminalId);
  }

  Widget _tabMenuBuilder(String peerId, CancelFunc cancelFunc) {
    final List<MenuEntryBase<String>> menu = [];
    const EdgeInsets padding = EdgeInsets.only(left: 8.0, right: 5.0);

    // New tab menu item
    menu.add(MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('New tab'),
        style: style,
      ),
      proc: () {
        _addNewTerminal(peerId);
        cancelFunc();
        // Also try to close any BotToast overlays
        BotToast.cleanAll();
      },
      padding: padding,
    ));

    menu.add(MenuEntryDivider());

    menu.add(MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate('Keep terminal sessions on disconnect'),
      getter: () async {
        final ffi = Get.find<FFI>(tag: 'terminal_$peerId');
        return bind.sessionGetToggleOptionSync(
          sessionId: ffi.sessionId,
          arg: kOptionTerminalPersistent,
        );
      },
      setter: (bool v) async {
        final ffi = Get.find<FFI>(tag: 'terminal_$peerId');
        await bind.sessionToggleOption(
          sessionId: ffi.sessionId,
          value: kOptionTerminalPersistent,
        );
      },
      padding: padding,
    ));

    return mod_menu.PopupMenu<String>(
      items: menu
          .map((e) => e.build(
                context,
                const MenuConfig(
                  commonColor: CustomPopupMenuTheme.commonColor,
                  height: CustomPopupMenuTheme.height,
                  dividerHeight: CustomPopupMenuTheme.dividerHeight,
                ),
              ))
          .expand((i) => i)
          .toList(),
    );
  }
}

extension _TerminalTabCreate on _TerminalTabPageState {
  TabInfo _createTerminalTab({
    required String peerId,
    required int terminalId,
    String? password,
    bool? isSharedPassword,
    bool? forceRelay,
    String? connToken,
  }) {
    final tabKey = '${peerId}_$terminalId';
    final alias = bind.mainGetPeerOptionSync(id: peerId, key: 'alias');
    final tabLabel =
        alias.isNotEmpty ? '$alias #$terminalId' : '$peerId #$terminalId';
    final clipboardSource = (
      peerId: peerId,
      terminalId: terminalId,
      tabKey: tabKey,
    );
    return TabInfo(
      key: tabKey,
      label: tabLabel,
      selectedIcon: _TerminalTabPageState.selectedIcon,
      unselectedIcon: _TerminalTabPageState.unselectedIcon,
      onTabCloseButton: () => _closeTab(tabKey),
      page: TerminalPage(
        key: ValueKey(tabKey),
        id: peerId,
        terminalId: terminalId,
        tabKey: tabKey,
        password: password,
        isSharedPassword: isSharedPassword,
        tabController: tabController,
        forceRelay: forceRelay,
        connToken: connToken,
        onClipboardWriteBlocked: _canHandleTerminalClipboardWriteRequest
            ? (text) => _handleTerminalClipboardWriteBlocked(
                  clipboardSource,
                  text,
                )
            : null,
        onClipboardWriteSucceeded: (_) {
          _handleTerminalClipboardWriteSucceeded(clipboardSource);
        },
      ),
    );
  }
}
