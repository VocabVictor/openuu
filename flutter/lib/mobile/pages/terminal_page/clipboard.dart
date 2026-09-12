part of 'terminal_page.dart';

extension _TerminalPageClipboard on _TerminalPageState {
  void _handleTerminalClipboardWriteBlocked(String clipboardText) {
    if (!mounted) return;
    final option = bind.mainGetLocalOption(
      key: kOptionAllowTerminalClipboardWrite,
    );
    final request = _terminalClipboardNotice.recordBlocked(
      source: widget.terminalId,
      text: clipboardText,
      option: option,
      canWrite: (_) => _canWriteTerminalClipboard,
    );
    if (request != null) _showTerminalClipboardNotice(request);
  }

  void _showTerminalClipboardNotice(
    TerminalClipboardNoticeRequest<int> request,
  ) {
    final controller = ScaffoldMessenger.of(context).showMaterialBanner(
      MaterialBanner(
        leading: const Icon(Icons.content_copy_outlined),
        content: Text(translate(kTerminalClipboardNoticeMessageKey)),
        actions: [
          AnimatedBuilder(
            animation: _terminalClipboardNotice,
            builder: (_, __) => TextButton(
              onPressed: _terminalClipboardNotice.canClaimAction
                  ? _handleTerminalClipboardNegativeAction
                  : null,
              child: Text(translate(request.negativeActionKey)),
            ),
          ),
          AnimatedBuilder(
            animation: _terminalClipboardNotice,
            builder: (_, __) => TextButton(
              onPressed: _terminalClipboardNotice.canClaimAction
                  ? _handleTerminalClipboardPositiveAction
                  : null,
              child: Text(translate(request.actionKey)),
            ),
          ),
        ],
      ),
    );
    _terminalClipboardNoticeController = controller;
    unawaited(controller.closed.then<void>((_) {
      if (identical(_terminalClipboardNoticeController, controller)) {
        _terminalClipboardNoticeController = null;
        _terminalClipboardNotice.noticeClosed();
      }
    }));
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

  bool get _canWriteTerminalClipboard =>
      _canHandleTerminalClipboardWriteRequest &&
      !_ffi.closed &&
      _ffi.ffiModel.permissions['clipboard'] != false;

  void _handleTerminalClipboardWriteSucceeded(String _) {
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
          '[TerminalPage] Failed to save terminal clipboard permission: $error');
      return;
    } finally {
      _terminalClipboardNotice.releaseAction();
    }
    _closeTerminalClipboardNotice();
  }

  Future<void> _completeTerminalClipboardWrite(
    TerminalClipboardNoticeRequest<int> request,
  ) async {
    var completed = false;
    try {
      completed = await completeTerminalClipboardWrite(
        clipboardText: request.text,
        canWrite: () => _canWriteTerminalClipboard,
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
          '[TerminalPage] Failed to complete terminal clipboard write: $error');
    } finally {
      _terminalClipboardNotice.releaseAction();
    }
    if (!completed) return;
    _closeTerminalClipboardNotice();
  }

  void _closeTerminalClipboardNotice() {
    if (!_terminalClipboardNotice.beginClose()) return;
    final controller = _terminalClipboardNoticeController;
    if (controller == null) {
      debugPrint('[TerminalPage] Clipboard notice controller is missing');
      _terminalClipboardNotice.noticeClosed();
      return;
    }
    controller.close();
  }
}
