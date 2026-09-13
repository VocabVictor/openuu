part of 'model.dart';

extension FfiSession on FFI {
  Future<void> onEvent2UIRgba() async {
    if (ffiModel.waitForImageDialogShow.isTrue) {
      ffiModel.waitForImageDialogShow.value = false;
      ffiModel.waitForImageTimer?.cancel();
      clearWaitingForImage(dialogManager, sessionId);
    }
    if (ffiModel.waitForFirstImage.value == true) {
      ffiModel.waitForFirstImage.value = false;
      ffiModel.cancelPendingRestoreTimer();
      ffiModel.resetRestartReconnectState();
      dialogManager.dismissAll();
      try {
        await canvasModel.updateViewStyle();
        await canvasModel.updateScrollStyle();
        await canvasModel.initializeEdgeScrollEdgeThickness();
        for (final cb in imageModel.callbacksOnFirstImage) {
          cb(id);
        }
      } finally {
        _applyPendingMonitorRestore();
      }
    }
  }

  void _applyPendingMonitorRestore() {
    final restore = ffiModel.pendingMonitorRestore;
    ffiModel._cancelPendingMonitorRestore();
    if (restore == null || closed) return;
    // The display list may have changed since the restore was queued.
    final displays = ffiModel.pi.displays;
    if ((restore == kAllDisplayValue && displays.isNotEmpty) ||
        (restore >= 0 && restore < displays.length)) {
      openMonitorInTheSameTab(restore, this, ffiModel.pi,
          recordSelection: false, updateCursorPos: false);
    }
  }

  /// Login with [password], choose if the client should [remember] it.
  void login(String osUsername, String osPassword, SessionID sessionId,
      String password, bool remember) {
    bind.sessionLogin(
        sessionId: sessionId,
        osUsername: osUsername,
        osPassword: osPassword,
        password: password,
        remember: remember);
  }

  void send2FA(SessionID sessionId, String code, bool trustThisDevice) {
    bind.sessionSend2Fa(
        sessionId: sessionId, code: code, trustThisDevice: trustThisDevice);
  }

  /// Close the remote session.
  Future<void> close({bool closeSession = true}) async {
    closed = true;
    chatModel.close();
    // Close all terminal models
    for (final model in _terminalModels.values) {
      model.dispose();
    }
    _terminalModels.clear();
    if (imageModel.image != null) {
      await setCanvasConfig(
          sessionId,
          cursorModel.x,
          cursorModel.y,
          canvasModel.x,
          canvasModel.y,
          canvasModel.scale,
          ffiModel.pi.currentDisplay);
    }
    imageModel.callbacksOnFirstImage.clear();
    await imageModel.update(null);
    cursorModel.clear();
    ffiModel.clear();
    canvasModel.clear();
    inputModel.resetModifiers();
    // Dispose relative mouse mode resources to ensure cursor is restored
    inputModel.disposeRelativeMouseMode();
    inputModel.disposeSideButtonTracking();
    inputModel.disposeMoveCoalescing();
    if (closeSession) {
      await bind.sessionClose(sessionId: sessionId);
    }
    debugPrint('model $id closed');
    id = '';
  }

  void setMethodCallHandler(FMethod callback) {
    platformFFI.setMethodCallHandler(callback);
  }

  Future<bool> invokeMethod(String method, [dynamic arguments]) async {
    return await platformFFI.invokeMethod(method, arguments);
  }

  Future<T?> invokeMethodWithResult<T>(String method,
      [dynamic arguments]) async {
    return await platformFFI.invokeMethodWithResult<T>(method, arguments);
  }

  // Terminal model management
  void registerTerminalModel(int terminalId, TerminalModel model) {
    debugPrint('[FFI] Registering terminal model for terminal $terminalId');
    _terminalModels[terminalId] = model;
  }

  void unregisterTerminalModel(int terminalId) {
    debugPrint('[FFI] Unregistering terminal model for terminal $terminalId');
    _terminalModels.remove(terminalId);
  }

  void routeTerminalResponse(Map<String, dynamic> evt) {
    final int terminalId = TerminalModel.getTerminalIdFromEvt(evt);

    // Route to specific terminal model if it exists
    final model = _terminalModels[terminalId];
    if (model != null) {
      model.handleTerminalResponse(evt);
    }
  }
}
