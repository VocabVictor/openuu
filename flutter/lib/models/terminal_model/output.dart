part of 'terminal_model.dart';

extension TerminalModelOutput on TerminalModel {
  void handleTerminalResponse(Map<String, dynamic> evt) {
    final String? type = evt['type'];
    final int evtTerminalId = TerminalModel.getTerminalIdFromEvt(evt);

    // Only handle events for this terminal
    if (evtTerminalId != terminalId) {
      debugPrint(
          '[TerminalModel] Ignoring event for terminal $evtTerminalId (not mine)');
      return;
    }

    switch (type) {
      case 'opened':
        _handleTerminalOpened(evt);
        break;
      case 'data':
        _handleTerminalData(evt);
        break;
      case 'closed':
        _handleTerminalClosed(evt);
        break;
      case 'error':
        _handleTerminalError(evt);
        break;
    }
  }

  void _handleTerminalOpened(Map<String, dynamic> evt) {
    final bool success = TerminalModel.getSuccessFromEvt(evt);
    final String message = evt['message']?.toString() ?? '';
    final String? serviceId = evt['service_id']?.toString();

    debugPrint(
        '[TerminalModel] Terminal opened response: success=$success, message=$message, service_id=$serviceId');

    if (success) {
      _terminalOpened = true;

      // On reconnect, the server may replay recent output. That replay can include
      // terminal queries like DSR/DA; xterm answers them through onOutput as
      // "^[[1;1R^[[2;2R^[[>0;0;0c", which must not be sent back to the peer.
      final replayTerminalOutput = evt['replay_terminal_output'];
      _suppressNextTerminalDataOutput = replayTerminalOutput == true ||
          message == 'Reconnected to existing terminal with pending output';

      // Fallback: if terminal view is not yet ready but already has valid
      // dimensions (e.g. layout completed before open response arrived),
      // mark view ready now to avoid output stuck in buffer indefinitely.
      if (!_terminalViewReady &&
          terminal.viewWidth > 0 &&
          terminal.viewHeight > 0) {
        _scheduleMarkViewReady();
      }

      // Process any buffered input
      _processBufferedInputAsync().then((_) {
        _notify();
      }).catchError((e) {
        debugPrint('[TerminalModel] Error processing buffered input: $e');
        _notify();
      });

      final persistentSessions =
          (evt['persistent_sessions'] as List<dynamic>? ?? [])
              .whereType<int>()
              .where((id) => !parent.terminalModels.containsKey(id))
              .toList();
      if (kWindowId != null && persistentSessions.isNotEmpty) {
        DesktopMultiWindow.invokeMethod(
            kWindowId!,
            kWindowEventRestoreTerminalSessions,
            jsonEncode({
              'peer_id': id,
              'persistent_sessions': persistentSessions,
            }));
      }
    } else {
      _writeToTerminal('Failed to open terminal: $message\r\n');
    }
  }

  Future<void> _processBufferedInputAsync() async {
    final buffer = List<String>.from(_inputBuffer);
    _inputBuffer.clear();

    for (final data in buffer) {
      try {
        await bind.sessionSendTerminalInput(
          sessionId: parent.sessionId,
          terminalId: terminalId,
          data: data,
        );
      } catch (e) {
        debugPrint('[TerminalModel] Error sending buffered input: $e');
      }
    }
  }

  void _handleTerminalData(Map<String, dynamic> evt) {
    final data = evt['data'];

    if (data != null) {
      final suppressTerminalOutput = _suppressNextTerminalDataOutput;
      _suppressNextTerminalDataOutput = false;
      try {
        String text = '';
        if (data is String) {
          // Try to decode as base64 first
          try {
            final bytes = base64Decode(data);
            text = utf8.decode(bytes, allowMalformed: true);
          } catch (e) {
            // If base64 decode fails, treat as plain text
            text = data;
          }
        } else if (data is List) {
          // Handle if data comes as byte array
          text = utf8.decode(List<int>.from(data), allowMalformed: true);
        } else {
          debugPrint('[TerminalModel] Unknown data type: ${data.runtimeType}');
          return;
        }

        _writeToTerminal(text, suppressTerminalOutput: suppressTerminalOutput);
      } catch (e) {
        debugPrint('[TerminalModel] Failed to process terminal data: $e');
      }
    }
  }

  /// Write text to terminal, buffering if the view is not yet ready.
  /// All terminal output should go through this method to avoid NaN errors
  /// from writing before the terminal view has valid layout dimensions.
  void _writeToTerminal(
    String text, {
    bool suppressTerminalOutput = false,
  }) {
    if (!_terminalViewReady) {
      // If a single chunk exceeds the cap, keep only its tail.
      // Note: truncation may split a multi-byte ANSI escape sequence,
      // which can cause a brief visual glitch on flush. This is acceptable
      // because it only affects the pre-layout buffering window and the
      // terminal will self-correct on subsequent output.
      if (text.length >= TerminalModel._kMaxOutputBufferChars) {
        final truncated = text.substring(text.length - TerminalModel._kMaxOutputBufferChars);
        _pendingOutputChunks
          ..clear()
          ..add(truncated);
        _pendingOutputSuppressFlags
          ..clear()
          ..add(suppressTerminalOutput);
        _pendingOutputSize = truncated.length;
      } else {
        _pendingOutputChunks.add(text);
        _pendingOutputSuppressFlags.add(suppressTerminalOutput);
        _pendingOutputSize += text.length;
        // Drop oldest chunks if exceeds limit (whole chunks to preserve ANSI sequences)
        while (_pendingOutputSize > TerminalModel._kMaxOutputBufferChars &&
            _pendingOutputChunks.length > 1) {
          final removed = _pendingOutputChunks.removeAt(0);
          _pendingOutputSuppressFlags.removeAt(0);
          _pendingOutputSize -= removed.length;
        }
      }
      return;
    }
    _writeTerminalChunk(text, suppressTerminalOutput: suppressTerminalOutput);
  }

  void _flushOutputBuffer() {
    if (_pendingOutputChunks.isEmpty) return;
    debugPrint(
        '[TerminalModel] Flushing $_pendingOutputSize buffered chars (${_pendingOutputChunks.length} chunks)');
    for (var i = 0; i < _pendingOutputChunks.length; i++) {
      _writeTerminalChunk(
        _pendingOutputChunks[i],
        suppressTerminalOutput: _pendingOutputSuppressFlags[i],
      );
    }
    _pendingOutputChunks.clear();
    _pendingOutputSuppressFlags.clear();
    _pendingOutputSize = 0;
  }

  void _writeTerminalChunk(
    String text, {
    required bool suppressTerminalOutput,
  }) {
    if (!suppressTerminalOutput) {
      terminal.write(text);
      return;
    }
    final previous = _suppressTerminalOutput;
    _suppressTerminalOutput = true;
    try {
      terminal.write(text);
    } finally {
      _suppressTerminalOutput = previous;
    }
  }

  /// Mark terminal view as ready and flush buffered output.
  void _scheduleMarkViewReady() {
    if (_disposed || _terminalViewReady || _markViewReadyScheduled) return;
    _markViewReadyScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _markViewReadyScheduled = false;
      if (_disposed || _terminalViewReady) return;
      if (terminal.viewWidth > 0 && terminal.viewHeight > 0) {
        _markViewReady();
      }
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  void _markViewReady() {
    if (_terminalViewReady) return;
    _terminalViewReady = true;
    _flushOutputBuffer();
  }

  void _handleTerminalClosed(Map<String, dynamic> evt) {
    final int exitCode = TerminalModel.getExitCodeFromEvt(evt);
    _writeToTerminal('\r\nTerminal closed with exit code: $exitCode\r\n');
    _terminalOpened = false;
    _notify();
    // Auto-close the tab/page
    onClosed?.call();
  }
}
