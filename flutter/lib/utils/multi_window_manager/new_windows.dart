part of 'multi_window_manager.dart';

extension MultiWindowManagerNewWindows on RustDeskMultiWindowManager {
  Future<MultiWindowCallResult> newSession(
    WindowType type,
    String methodName,
    String remoteId,
    List<int> windows, {
    String? quickLaunch,
    bool viewOnly = false,
    String? password,
    bool? forceRelay,
    String? switchUuid,
    bool? isRDP,
    bool? isSharedPassword,
    String? connToken,
  }) async {
    var params = {
      "type": type.index,
      "id": remoteId,
      "quickLaunch": quickLaunch,
      "viewOnly": viewOnly,
      "password": password,
      "forceRelay": forceRelay
    };
    if (switchUuid != null) {
      params['switch_uuid'] = switchUuid;
    }
    if (isRDP != null) {
      params['isRDP'] = isRDP;
    }
    if (isSharedPassword != null) {
      params['isSharedPassword'] = isSharedPassword;
    }
    if (connToken != null) {
      params['connToken'] = connToken;
    }
    final msg = jsonEncode(params);
    // Never activate an existing control session for a view-only request.
    if (quickLaunch != null) {
      for (final windowId in List<int>.from(windows)) {
        if (await DesktopMultiWindow.invokeMethod(windowId, 'quick_launch',
            {'id': remoteId, 'app': quickLaunch}) == true) {
          return MultiWindowCallResult(windowId, null);
        }
      }
    }
    if (viewOnly || quickLaunch != null) {
      final windowId = await newSessionWindow(type, remoteId, msg, windows, false);
      return MultiWindowCallResult(windowId, null);
    }

    // separate window for file transfer is not supported
    bool openInTabs = type != WindowType.RemoteDesktop ||
        mainGetLocalBoolOptionSync(kOptionOpenNewConnInTabs);

    if (windows.length > 1 || !openInTabs) {
      for (final windowId in windows) {
        if (await DesktopMultiWindow.invokeMethod(
            windowId, kWindowEventActiveSession, remoteId)) {
          return MultiWindowCallResult(windowId, null);
        }
      }
    }

    return _newSession(openInTabs, type, methodName, remoteId, windows, msg);
  }

  Future<MultiWindowCallResult> newRemoteDesktop(
    String remoteId, {
    String? quickLaunch,
    bool viewOnly = false,
    String? password,
    bool? isSharedPassword,
    String? switchUuid,
    bool? forceRelay,
  }) async {
    return await newSession(
      WindowType.RemoteDesktop,
      kWindowEventNewRemoteDesktop,
      remoteId,
      _remoteDesktopWindows,
      quickLaunch: quickLaunch,
      viewOnly: viewOnly,
      password: password,
      forceRelay: forceRelay,
      switchUuid: switchUuid,
      isSharedPassword: isSharedPassword,
    );
  }

  Future<MultiWindowCallResult> newFileTransfer(
    String remoteId, {
    String? password,
    bool? isSharedPassword,
    bool? forceRelay,
    String? connToken,
  }) async {
    return await newSession(
      WindowType.FileTransfer,
      kWindowEventNewFileTransfer,
      remoteId,
      _fileTransferWindows,
      password: password,
      forceRelay: forceRelay,
      isSharedPassword: isSharedPassword,
      connToken: connToken,
    );
  }

  Future<MultiWindowCallResult> newViewCamera(
    String remoteId, {
    String? password,
    bool? isSharedPassword,
    String? switchUuid,
    bool? forceRelay,
    String? connToken,
  }) async {
    return await newSession(
      WindowType.ViewCamera,
      kWindowEventNewViewCamera,
      remoteId,
      _viewCameraWindows,
      password: password,
      forceRelay: forceRelay,
      switchUuid: switchUuid,
      isSharedPassword: isSharedPassword,
      connToken: connToken,
    );
  }

  Future<MultiWindowCallResult> newPortForward(
    String remoteId,
    bool isRDP, {
    String? password,
    bool? isSharedPassword,
    bool? forceRelay,
    String? connToken,
  }) async {
    return await newSession(
      WindowType.PortForward,
      kWindowEventNewPortForward,
      remoteId,
      _portForwardWindows,
      password: password,
      forceRelay: forceRelay,
      isRDP: isRDP,
      isSharedPassword: isSharedPassword,
      connToken: connToken,
    );
  }

  Future<MultiWindowCallResult> newTerminal(
    String remoteId, {
    String? password,
    bool? isSharedPassword,
    bool? forceRelay,
    String? connToken,
  }) async {
    // Iterate through terminal windows in reverse order to prioritize
    // the most recently added or used windows, as they are more likely
    // to have an active session.
    for (final windowId in _terminalWindows.reversed) {
      if (await DesktopMultiWindow.invokeMethod(
          windowId, kWindowEventActiveSession, remoteId)) {
        return MultiWindowCallResult(windowId, null);
      }
    }

    // Terminal windows should always create new windows, not reuse
    // This avoids the MissingPluginException when trying to invoke
    // new_terminal on an inactive window
    var params = {
      "type": WindowType.Terminal.index,
      "id": remoteId,
      "password": password,
      "forceRelay": forceRelay,
      "isSharedPassword": isSharedPassword,
      "connToken": connToken,
    };
    final msg = jsonEncode(params);

    // Always create a new window for terminal
    final windowId = await newSessionWindow(
        WindowType.Terminal, remoteId, msg, _terminalWindows, false);
    return MultiWindowCallResult(windowId, null);
  }
}
