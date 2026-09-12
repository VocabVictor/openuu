part of 'terminal_tab_page.dart';

extension _TerminalTabClipboard on _TerminalTabPageState {
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
}
