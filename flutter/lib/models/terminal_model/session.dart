part of 'terminal_model.dart';

extension TerminalModelSession on TerminalModel {
  void onReady() {
    parent.dialogManager.dismissAll();

    // Fire and forget - don't block onReady. If the transport reconnects while
    // this model is still open, re-send OpenTerminal so the remote service marks
    // the persistent session active again and resumes output streaming.
    openTerminal(force: _terminalOpened).catchError((e) {
      debugPrint('[TerminalModel] Error opening terminal: $e');
    });
  }

  Future<void> openTerminal({bool force = false}) async {
    if (_terminalOpened && !force) return;
    // Request the remote side to open a terminal with default shell
    // The remote side will decide which shell to use based on its OS

    // Get terminal dimensions, ensuring they are valid
    int rows = 24;
    int cols = 80;

    if (terminal.viewHeight > 0) {
      rows = terminal.viewHeight;
    }
    if (terminal.viewWidth > 0) {
      cols = terminal.viewWidth;
    }

    debugPrint(
        '[TerminalModel] Opening terminal $terminalId, sessionId: ${parent.sessionId}, size: ${cols}x$rows');
    try {
      await bind
          .sessionOpenTerminal(
        sessionId: parent.sessionId,
        terminalId: terminalId,
        rows: rows,
        cols: cols,
      )
          .timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          throw TimeoutException(
              'sessionOpenTerminal timed out after 5 seconds');
        },
      );
      debugPrint('[TerminalModel] sessionOpenTerminal called successfully');
    } catch (e) {
      debugPrint('[TerminalModel] Error calling sessionOpenTerminal: $e');
      // Optionally show error to user
      if (e is TimeoutException) {
        _writeToTerminal('Failed to open terminal: Connection timeout\r\n');
      }
    }
  }

  Future<void> sendVirtualKey(String data) async {
    return _handleInput(data);
  }

  Future<void> pasteText(String data) async {
    final payload = prepareTerminalInputPayload(
      data,
      source: TerminalInputSource.paste,
      isMobileOrWebMobile: false,
      bracketedPasteMode: terminal.bracketedPasteMode,
      ctrlLocked: false,
      altLocked: false,
    );
    return _sendInputPayload(payload);
  }

  Future<void> closeTerminal() async {
    if (_terminalOpened) {
      try {
        await bind
            .sessionCloseTerminal(
          sessionId: parent.sessionId,
          terminalId: terminalId,
        )
            .timeout(
          const Duration(seconds: 3),
          onTimeout: () {
            throw TimeoutException(
                'sessionCloseTerminal timed out after 3 seconds');
          },
        );
        debugPrint('[TerminalModel] sessionCloseTerminal called successfully');
      } catch (e) {
        debugPrint('[TerminalModel] Error calling sessionCloseTerminal: $e');
        // Continue with cleanup even if close fails
      }
      _terminalOpened = false;
      _notify();
    }
  }
}

extension TerminalModelInput on TerminalModel {
  Future<void> _handleInput(String data) async {
    // xterm can complete asynchronous input after the Flutter page has gone
    // away. Stop before reading or clearing widget-owned modifier state.
    if (_disposed) return;

    // Soft keyboards (notably iOS) emit '\n' when Enter is pressed, while a
    // real keyboard's Enter sends '\r'. Some Android keyboards also emit '\n'.
    // - Peer Windows: '\r' works, '\n' is just a newline.
    // - Peer Linux: canonical-mode shells accept both, but raw-mode apps
    //   (readline, prompt_toolkit, vim, TUI frameworks) expect '\r'.
    // - Peer macOS: same as Linux, raw-mode apps expect '\r'
    //   (https://github.com/rustdesk/rustdesk/issues/14907).
    // So on mobile / web-mobile, normalize the original lone '\n' to '\r'
    // before modifier mappings. This keeps Ctrl+J mapped to LF instead of
    // having the generated control code rewritten to CR afterward.
    // Multi-character keyboard payloads, such as terminal escape sequences,
    // remain unchanged. Paste input follows a separate preprocessing path.
    final ctrlLocked = isCtrlLocked?.call() ?? false;
    final altLocked = isAltLocked?.call() ?? false;
    final modifiersActive = ctrlLocked || altLocked;
    // Use the same predicate for transformation and consumption. Control keys
    // and escape sequences must not silently consume a pending one-shot lock.
    final shouldConsumeModifiers =
        modifiersActive && shouldApplyTerminalInputModifiers(data);
    data = prepareTerminalInputPayload(
      data,
      // IME soft-keyboard paste prompts currently arrive from xterm as normal
      // text input with no paste-origin metadata. Keep them on the keyboard path;
      // clipboard-content heuristics can misclassify ordinary typing.
      source: TerminalInputSource.keyboard,
      isMobileOrWebMobile: isMobile,
      bracketedPasteMode: terminal.bracketedPasteMode,
      ctrlLocked: ctrlLocked,
      altLocked: altLocked,
    );
    if (shouldConsumeModifiers) {
      if (ctrlLocked) clearCtrlLock?.call();
      if (altLocked) clearAltLock?.call();
    }
    return _sendInputPayload(data);
  }

  /// Sends an already prepared payload without applying keyboard semantics.
  /// Both normal input and paste use this transport path after their source-
  /// specific preprocessing has completed.
  Future<void> _sendInputPayload(String data) async {
    // Clipboard reads and native sends may complete after the terminal page has
    // closed. Never send or re-buffer input once this model is disposed.
    if (_disposed) return;

    if (_terminalOpened) {
      // Send user input to remote terminal
      try {
        await bind.sessionSendTerminalInput(
          sessionId: parent.sessionId,
          terminalId: terminalId,
          data: data,
        );
      } catch (e) {
        debugPrint('[TerminalModel] Error sending terminal input: $e');
      }
    } else {
      debugPrint('[TerminalModel] Terminal not opened yet, buffering input');
      _inputBuffer.add(data);
    }
  }
}
