part of 'model.dart';

extension FfiModelMsgBox on FfiModel {
  /// Handle the message box event based on [evt] and [id].
  handleMsgBox(Map<String, dynamic> evt, SessionID sessionId, String peerId) {
    if (parent.target == null) return;
    final dialogManager = parent.target!.dialogManager;
    final type = evt['type'];
    final title = evt['title'];
    final text = evt['text'];
    final link = evt['link'];

    // The peer-gone detector reconnects under `restarting-show` rather than an error title, so
    // it needs naming here too. By its own title, not the type: an explicitly restarted remote
    // device reaches the same type from a path this change does not touch.
    if (isAndroid &&
        _androidDocumentPickerActive &&
        (title == 'Connection Error' ||
            (type == 'restarting-show' && title == 'Connecting...'))) {
      _androidDocumentPickerInterruptedConnection = true;
      return;
    }

    // Disable relative mouse mode on any error-type message to ensure cursor is released.
    // This includes connection errors, session-ending messages, elevation errors, etc.
    // Safety: releasing pointer lock on errors prevents the user from being stuck.
    if (title == 'Connection Error' ||
        type == 'error' ||
        type == 'restarting' ||
        (type is String && type.contains('error'))) {
      parent.target?.inputModel.setRelativeMouseMode(false);
    }

    if (type == 're-input-password') {
      wrongPasswordDialog(sessionId, dialogManager, type, title, text);
    } else if (type == 'input-2fa') {
      enter2FaDialog(sessionId, dialogManager);
    } else if (type == 'input-password') {
      enterPasswordDialog(sessionId, dialogManager);
    } else if (type == 'terminal-admin-login') {
      enterUserLoginDialog(
          sessionId, dialogManager, 'terminal-admin-login-tip');
    } else if (type == 'terminal-admin-login-password') {
      enterUserLoginAndPasswordDialog(
          sessionId, dialogManager, 'terminal-admin-login-tip');
    } else if (type == 'restarting') {
      // Treat restart messages as reconnect control events. Rust still sends
      // title/text for legacy UI and translation reuse; Flutter keeps the last
      // frame briefly, then shows the Connecting overlay.
      if (_restartReconnectDelayTimer == null) {
        parent.target?.inputModel.setRelativeMouseMode(false);
        _cancelPendingMonitorRestore();
        bind.sessionReconnect(sessionId: sessionId, forceRelay: false);
        clearPermissions();
        // Retry once more after the silent window so restart reconnect attempts
        // are spaced by the empirical short cadence instead of only updating UI.
        _restartReconnectDelayTimer =
            Timer(Duration(seconds: _restartReconnectSilentDelaySecs), () {
          _restartReconnectDelayTimer = null;
          if (parent.target?.closed == true) {
            return;
          }
          reconnect(dialogManager, sessionId, false);
        });
      }
    } else if (type == 'restarting-show') {
      _restartReconnectDelayTimer?.cancel();
      _restartReconnectDelayTimer = null;
      reconnect(dialogManager, sessionId, false);
    } else if (type == 'wait-remote-accept-nook') {
      showWaitAcceptDialog(sessionId, type, title, text, dialogManager);
    } else if (type == 'on-uac' || type == 'on-foreground-elevated') {
      showOnBlockDialog(sessionId, type, title, text, dialogManager);
    } else if (type == 'wait-uac') {
      showWaitUacDialog(sessionId, dialogManager, type);
    } else if (type == 'elevation-error') {
      showElevationError(sessionId, type, title, text, dialogManager);
    } else if (type == 'relay-hint' || type == 'relay-hint2') {
      showRelayHintDialog(sessionId, type, title, text, dialogManager, peerId);
    } else if (text == kMsgboxTextWaitingForImage) {
      showConnectedWaitingForImage(dialogManager, sessionId, type, title, text);
    } else if (title == 'Privacy mode') {
      final hasRetry = evt['hasRetry'] == 'true';
      showPrivacyFailedDialog(
          sessionId, type, title, text, link, hasRetry, dialogManager);
    } else {
      var hasRetry = evt['hasRetry'] == 'true';
      if (!hasRetry) {
        hasRetry = shouldAutoRetryOnOffline(type, title, text);
      }
      showMsgBox(sessionId, type, title, text, link, hasRetry, dialogManager);
    }
  }

  void resetRestartReconnectState() {
    _restartReconnectDelayTimer?.cancel();
    _restartReconnectDelayTimer = null;
  }

  void beginAndroidDocumentPicker() {
    if (!isAndroid) return;
    _androidDocumentPickerActive = true;
    _androidDocumentPickerInterruptedConnection = false;
  }

  void endAndroidDocumentPicker() {
    if (!isAndroid) return;
    _androidDocumentPickerActive = false;
    if (!_androidDocumentPickerInterruptedConnection ||
        parent.target?.closed == true) {
      return;
    }
    _androidDocumentPickerInterruptedConnection = false;
    reconnect(parent.target!.dialogManager, sessionId, false);
  }

  /// Auto-retry check for "Remote desktop is offline" error.
  /// returns true to auto-retry, false otherwise.
  bool shouldAutoRetryOnOffline(
    String type,
    String title,
    String text,
  ) {
    if (type == 'error' &&
        title == 'Connection Error' &&
        text == 'Remote desktop is offline' &&
        _pi.isSet.isTrue) {
      // Auto retry for ~30s (server's peer offline threshold) when controlled peer's account changes
      // (e.g., signout, switch user, login into OS) causes temporary offline via websocket/tcp connection.
      // The actual wait may exceed 30s (e.g., 20s elapsed + 16s next retry = 36s), which is acceptable
      // since the controlled side reconnects quickly after account changes.
      // Uses time-based check instead of _reconnects count because user can manually retry.
      // https://github.com/rustdesk/rustdesk/discussions/14048
      if (_offlineReconnectStartTime == null) {
        // First offline, record time and start retry
        _offlineReconnectStartTime = DateTime.now();
        return true;
      } else {
        final elapsed =
            DateTime.now().difference(_offlineReconnectStartTime!).inSeconds;
        if (elapsed < 30) {
          return true;
        }
      }
    }
    return false;
  }

  handleToast(Map<String, dynamic> evt, SessionID sessionId, String peerId) {
    final type = evt['type'] ?? 'info';
    final text = evt['text'] ?? '';
    final durMsc = evt['dur_msec'] ?? 2000;
    final duration = Duration(milliseconds: durMsc);
    if ((text).isEmpty) {
      BotToast.showLoading(
        duration: duration,
        clickClose: true,
        allowClick: true,
      );
    } else {
      if (type.contains('error')) {
        BotToast.showText(
          contentColor: Colors.red,
          text: translate(text),
          duration: duration,
          clickClose: true,
          onlyOne: true,
        );
      } else {
        BotToast.showText(
          text: translate(text),
          duration: duration,
          clickClose: true,
          onlyOne: true,
        );
      }
    }
  }

  /// Show a message box with [type], [title] and [text].
  showMsgBox(SessionID sessionId, String type, String title, String text,
      String link, bool hasRetry, OverlayDialogManager dialogManager,
      {bool? hasCancel}) async {
    // A session window with a status bar counts the reconnect down in the bar
    // instead of opening a dialog over the last frame.
    final statusBar = SessionStatusRegistry.find(parent.target?.id ?? '');
    if (hasRetry && statusBar != null) {
      _timer?.cancel();
      _timer = null;
      statusBar.disconnected(
          seconds: _reconnects,
          onTimeout: () => reconnect(dialogManager, sessionId, false));
      _reconnects *= 2;
      return;
    }
    final noteAllowed = parent.target != null &&
        allowAskForNoteAtEndOfConnection(parent.target, false) &&
        (title == "Connection Error" || type == "restarting");
    final showNoteEdit = noteAllowed && !hasRetry;
    if (showNoteEdit) {
      await showConnEndAuditDialogCloseCanceled(
          ffi: parent.target!, type: type, title: title, text: text);
      closeConnection();
    } else {
      VoidCallback? onSubmit;
      if (noteAllowed && hasRetry) {
        final ffi = parent.target!;
        onSubmit = () async {
          _timer?.cancel();
          _timer = null;
          await showConnEndAuditDialogCloseCanceled(
              ffi: ffi, type: type, title: title, text: text);
          closeConnection();
        };
      }
      msgBox(sessionId, type, title, text, link, dialogManager,
          hasCancel: hasCancel,
          reconnect: hasRetry ? reconnect : null,
          reconnectTimeout: hasRetry ? _reconnects : null,
          onSubmit: onSubmit);
    }
    _timer?.cancel();
    if (hasRetry) {
      _timer = Timer(Duration(seconds: _reconnects), () {
        reconnect(dialogManager, sessionId, false);
      });
      _reconnects *= 2;
    } else {
      _reconnects = 1;
      _offlineReconnectStartTime = null;
    }
  }
}
