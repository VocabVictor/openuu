part of 'model.dart';

extension FfiModelReconnect on FfiModel {
  void _cancelPendingMonitorRestore() {
    _pendingRestoreTimer?.cancel();
    _pendingRestoreTimer = null;
    pendingMonitorRestore = null;
  }

  void cancelPendingRestoreTimer() {
    _pendingRestoreTimer?.cancel();
    _pendingRestoreTimer = null;
  }

  void reconnect(OverlayDialogManager dialogManager, SessionID sessionId,
      bool forceRelay) {
    // Disable relative mouse mode before reconnecting to ensure cursor is released.
    parent.target?.inputModel.setRelativeMouseMode(false);
    _cancelPendingMonitorRestore();
    bind.sessionReconnect(sessionId: sessionId, forceRelay: forceRelay);
    clearPermissions();
    dialogManager.dismissAll();
    dialogManager.showLoading(translate('Connecting...'),
        onCancel: closeConnection);
  }

  Future<void> showRelayHintDialog(
      SessionID sessionId,
      String type,
      String title,
      String text,
      OverlayDialogManager dialogManager,
      String peerId) async {
    var hint = "\n\n${translate('relay_hint_tip')}";
    if (text.contains("10054") || text.contains("104")) {
      hint = "";
    }
    final text2 = "${translate(text)}$hint";

    if (parent.target != null &&
        allowAskForNoteAtEndOfConnection(parent.target, false) &&
        this.pi.isSet.isTrue) {
      if (await showConnEndAuditDialogCloseCanceled(
          ffi: parent.target!, type: type, title: title, text: text2)) {
        return;
      }
      closeConnection();
      return;
    }

    dialogManager.show(tag: '$sessionId-$type', (setState, close, context) {
      onClose() {
        closeConnection();
        close();
      }

      final style =
          ElevatedButton.styleFrom(backgroundColor: Colors.green[700]);

      return CustomAlertDialog(
        title: null,
        content: msgboxContent(type, title, text2),
        actions: [
          dialogButton('Close', onPressed: onClose, isOutline: true),
          if (type == 'relay-hint')
            dialogButton('Connect via relay',
                onPressed: () => reconnect(dialogManager, sessionId, true),
                buttonStyle: style,
                isOutline: true),
          dialogButton('Retry',
              onPressed: () => reconnect(dialogManager, sessionId, false)),
          if (type == 'relay-hint2')
            dialogButton('Connect via relay',
                onPressed: () => reconnect(dialogManager, sessionId, true),
                buttonStyle: style),
        ],
        onCancel: onClose,
      );
    });
  }

  void showConnectedWaitingForImage(OverlayDialogManager dialogManager,
      SessionID sessionId, String type, String title, String text) {
    onClose() {
      closeConnection();
    }

    if (waitForFirstImage.isFalse) return;
    dialogManager.show(
      (setState, close, context) => CustomAlertDialog(
          title: null,
          content: SelectionArea(child: msgboxContent(type, title, text)),
          actions: [
            dialogButton("Cancel", onPressed: onClose, isOutline: true)
          ],
          onCancel: onClose),
      tag: '$sessionId-waiting-for-image',
    );
    waitForImageDialogShow.value = true;
    waitForImageTimer = Timer(Duration(milliseconds: 1500), () {
      if (waitForFirstImage.isTrue && !isRefreshing) {
        bind.sessionInputOsPassword(sessionId: sessionId, value: '');
      }
    });
    bind.sessionOnWaitingForImageDialogShow(sessionId: sessionId);
  }

  void showPrivacyFailedDialog(
      SessionID sessionId,
      String type,
      String title,
      String text,
      String link,
      bool hasRetry,
      OverlayDialogManager dialogManager) {
    // There are display changes on the remote side,
    // which will cause some messages to refresh the canvas and dismiss dialogs.
    // So we add a delay here to ensure the dialog is displayed.
    Future.delayed(Duration(milliseconds: 3000), () {
      showMsgBox(sessionId, type, title, text, link, hasRetry, dialogManager);
    });
  }

  _updateSessionWidthHeight(SessionID sessionId) {
    if (_rect == null) return;
    if (_rect!.width <= 0 || _rect!.height <= 0) {
      debugPrintStack(
          label: 'invalid display size (${_rect!.width},${_rect!.height})');
    } else {
      final displays = _pi.getCurDisplays();
      if (displays.length == 1) {
        bind.sessionSetSize(
          sessionId: sessionId,
          display:
              this.pi.currentDisplay == kAllDisplayValue ? 0 : this.pi.currentDisplay,
          width: displays[0].width,
          height: displays[0].height,
        );
      } else {
        for (int i = 0; i < displays.length; ++i) {
          bind.sessionSetSize(
            sessionId: sessionId,
            display: i,
            width: displays[i].width,
            height: displays[i].height,
          );
        }
      }
    }
  }

  void _queryAuditGuid(String peerId) async {
    try {
      if (bind.isDisableAccount()) {
        return;
      }
      if (bind
          .sessionGetAuditServerSync(sessionId: sessionId, typ: "conn/active")
          .isEmpty) {
        return;
      }
      if (!mainGetLocalBoolOptionSync(
          kOptionAllowAskForNoteAtEndOfConnection)) {
        return;
      }
      if (bind.sessionGetAuditGuid(sessionId: sessionId).isNotEmpty) {
        debugPrint('Get cached audit GUID');
        return;
      }
      final url = bind.sessionGetAuditServerSync(
          sessionId: sessionId, typ: "conn/active");
      if (url.isEmpty) {
        return;
      }
      final initialConnSessionId =
          bind.sessionGetConnSessionId(sessionId: sessionId);
      final connType = switch (parent.target?.connType) {
        ConnType.defaultConn => 0,
        ConnType.fileTransfer => 1,
        ConnType.portForward => 2,
        ConnType.rdp => 2,
        ConnType.viewCamera => 3,
        ConnType.terminal => 4,
        _ => 0,
      };

      const retryIntervals = [1, 1, 2, 2, 3, 3];

      for (int attempt = 1; attempt <= retryIntervals.length; attempt++) {
        final currentConnSessionId =
            bind.sessionGetConnSessionId(sessionId: sessionId);
        if (currentConnSessionId != initialConnSessionId) {
          debugPrint('connSessionId changed, stopping audit GUID query');
          return;
        }

        final fullUrl =
            '$url?id=$peerId&session_id=$currentConnSessionId&conn_type=$connType';

        debugPrint(
            'Querying audit GUID, attempt $attempt/${retryIntervals.length}');
        try {
          var headers = getHttpHeaders();
          headers['Content-Type'] = "application/json";

          final response = await http.get(
            Uri.parse(fullUrl),
            headers: headers,
          );

          if (response.statusCode == 200) {
            final guid = jsonDecode(response.body) as String?;
            if (guid != null && guid.isNotEmpty) {
              bind.sessionSetAuditGuid(sessionId: sessionId, guid: guid);
              debugPrint('Successfully retrieved audit GUID');
              return;
            }
          } else {
            debugPrint(
                'Failed to query audit GUID. Status: ${response.statusCode}, Body: ${response.body}');
            return;
          }
        } catch (e) {
          debugPrint('Error querying audit GUID (attempt $attempt): $e');
        }

        if (attempt < retryIntervals.length) {
          await Future.delayed(Duration(seconds: retryIntervals[attempt - 1]));
        }
      }

      debugPrint(
          'Failed to retrieve audit GUID after ${retryIntervals.length} attempts');
    } catch (e) {
      debugPrint('Error in _queryAuditGuid: $e');
    }
  }
}
