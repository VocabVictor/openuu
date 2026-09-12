part of 'terminal_tab_page.dart';

extension _TerminalTabWindow on _TerminalTabPageState {
  Widget _buildAddButton() {
    return Row(mainAxisSize: MainAxisSize.min, children: [
      ActionIcon(message: 'Terminal sessions', icon: Icons.list_alt,
        onTap: () => _setState(() => _showSessions = true), isClose: false),
      ActionIcon(
      message: 'New tab',
      icon: IconFont.add,
      onTap: () {
        _addNewTerminalForCurrentPeer();
      },
      isClose: false,
    )]);
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.state.value.tabs.length;
    if (connLength == 1) {
      if (await desktopTryShowTabAuditDialogCloseCancelled(
        id: tabController.state.value.tabs[0].key,
        tabController: tabController,
      )) {
        return false;
      }
    }
    if (connLength <= 1) {
      await _closeAllTabs();
      return true;
    } else {
      final bool res;
      if (!option2bool(kOptionEnableConfirmClosingTabs,
          bind.mainGetLocalOption(key: kOptionEnableConfirmClosingTabs))) {
        res = true;
      } else {
        res = await closeConfirmDialog();
      }
      if (res) {
        await _closeAllTabs();
      }
      return res;
    }
  }
}

extension _TerminalTabSessions on _TerminalTabPageState {
  Future<void> _restoreSessions(String arguments) async {
    Map<String, dynamic>? args;
    try {
      args = jsonDecode(arguments) as Map<String, dynamic>;
    } catch (e) {
      debugPrint("Error parsing JSON arguments in _restoreSessions: $e");
      return;
    }
    final persistentSessions =
        args['persistent_sessions'] as List<dynamic>? ?? [];
    final sortedSessions = persistentSessions.whereType<int>().toList()..sort();
    var peerId = args['peer_id'] as String? ?? '';
    if (peerId.isEmpty) {
      if (tabController.state.value.tabs.isEmpty ||
          tabController.state.value.selected >=
              tabController.state.value.tabs.length) {
        debugPrint('[TerminalTabPage] Skip restore: no selected tab');
        return;
      }
      final currentTab = tabController.state.value.selectedTabInfo;
      final parsed = _parseTabKey(currentTab.key);
      if (parsed == null) return;
      peerId = parsed.$1;
    }
    final existingTerminalIds = tabController.state.value.tabs
        .map((tab) => _parseTabKey(tab.key))
        .where((parsed) => parsed != null && parsed.$1 == peerId)
        .map((parsed) => parsed!.$2)
        .toSet();
    if (existingTerminalIds.isEmpty) {
      debugPrint(
          '[TerminalTabPage] Skip restore: no seed tab for peer $peerId');
      return;
    }
    for (final terminalId in sortedSessions) {
      if (!existingTerminalIds.add(terminalId)) {
        continue;
      }
      _addNewTerminal(peerId, terminalId: terminalId);
      // A delay is required to ensure the UI has sufficient time to update
      // before adding the next terminal. Without this delay, `_TerminalPageState::dispose()`
      // may be called prematurely while the tab widget is still in the tab controller.
      // This behavior is likely due to a race condition between the UI rendering lifecycle
      // and the addition of new tabs. Attempts to use `_TerminalPageState::addPostFrameCallback()`
      // to wait for the previous page to be ready were unsuccessful, as the observed call sequence is:
      // `initState() 2 -> dispose() 2 -> postFrameCallback() 2`, followed by `initState() 3`.
      // The `Future.delayed` approach mitigates this issue by introducing a buffer period,
      // allowing the UI to stabilize before proceeding.
      await Future.delayed(const Duration(milliseconds: 300));
    }
  }

  bool _handleKeyEvent(KeyEvent event) {
    if (_showSessions) return false;
    if (event is KeyDownEvent) {
      // Use Cmd+T on macOS, Ctrl+Shift+T on other platforms
      if (event.logicalKey == LogicalKeyboardKey.keyT) {
        if (isMacOS &&
            HardwareKeyboard.instance.isMetaPressed &&
            !HardwareKeyboard.instance.isShiftPressed) {
          // macOS: Cmd+T (standard for new tab)
          _addNewTerminalForCurrentPeer();
          return true;
        } else if (!isMacOS &&
            HardwareKeyboard.instance.isControlPressed &&
            HardwareKeyboard.instance.isShiftPressed) {
          // Other platforms: Ctrl+Shift+T (to avoid conflict with Ctrl+T in terminal)
          _addNewTerminalForCurrentPeer();
          return true;
        }
      }

      // Use Cmd+W on macOS, Ctrl+Shift+W on other platforms
      if (event.logicalKey == LogicalKeyboardKey.keyW) {
        if (isMacOS &&
            HardwareKeyboard.instance.isMetaPressed &&
            !HardwareKeyboard.instance.isShiftPressed) {
          // macOS: Cmd+W (standard for close tab)
          final currentTab = tabController.state.value.selectedTabInfo;
          if (tabController.state.value.tabs.length > 1) {
            _closeTab(currentTab.key);
            return true;
          }
        } else if (!isMacOS &&
            HardwareKeyboard.instance.isControlPressed &&
            HardwareKeyboard.instance.isShiftPressed) {
          // Other platforms: Ctrl+Shift+W (to avoid conflict with Ctrl+W word delete)
          final currentTab = tabController.state.value.selectedTabInfo;
          if (tabController.state.value.tabs.length > 1) {
            _closeTab(currentTab.key);
            return true;
          }
        }
      }

      // Use Alt+Left/Right for tab navigation (avoids conflicts)
      if (HardwareKeyboard.instance.isAltPressed) {
        if (event.logicalKey == LogicalKeyboardKey.arrowLeft) {
          // Previous tab
          final currentIndex = tabController.state.value.selected;
          if (currentIndex > 0) {
            tabController.jumpTo(currentIndex - 1);
          }
          return true;
        } else if (event.logicalKey == LogicalKeyboardKey.arrowRight) {
          // Next tab
          final currentIndex = tabController.state.value.selected;
          if (currentIndex < tabController.length - 1) {
            tabController.jumpTo(currentIndex + 1);
          }
          return true;
        }
      }

      // Check for Cmd/Ctrl + Number (switch to specific tab)
      final numberKeys = [
        LogicalKeyboardKey.digit1,
        LogicalKeyboardKey.digit2,
        LogicalKeyboardKey.digit3,
        LogicalKeyboardKey.digit4,
        LogicalKeyboardKey.digit5,
        LogicalKeyboardKey.digit6,
        LogicalKeyboardKey.digit7,
        LogicalKeyboardKey.digit8,
        LogicalKeyboardKey.digit9,
      ];

      for (int i = 0; i < numberKeys.length; i++) {
        if (event.logicalKey == numberKeys[i] &&
            ((isMacOS && HardwareKeyboard.instance.isMetaPressed) ||
                (!isMacOS && HardwareKeyboard.instance.isControlPressed))) {
          if (i < tabController.length) {
            tabController.jumpTo(i);
            return true;
          }
        }
      }
    }
    return false;
  }

  void _addNewTerminal(String peerId, {int? terminalId}) {
    // Find first tab for this peer to get connection parameters
    final firstTab = tabController.state.value.tabs.firstWhere(
      (tab) {
        final last = tab.key.lastIndexOf('_');
        return last > 0 && tab.key.substring(0, last) == peerId;
      },
    );
    if (firstTab.page is TerminalPage) {
      final page = firstTab.page as TerminalPage;
      final newTerminalId = terminalId ?? _nextTerminalId++;
      if (terminalId != null && terminalId >= _nextTerminalId) {
        _nextTerminalId = terminalId + 1;
      }
      tabController.add(_createTerminalTab(
        peerId: peerId,
        terminalId: newTerminalId,
        password: page.password,
        isSharedPassword: page.isSharedPassword,
        forceRelay: page.forceRelay,
        connToken: page.connToken,
      ));
    }
  }
}
