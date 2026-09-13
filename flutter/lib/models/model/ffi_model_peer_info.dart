part of 'model.dart';

extension FfiModelPeerInfo on FfiModel {
  /// Handle the peer info event based on [evt].
  handlePeerInfo(Map<String, dynamic> evt, String peerId, bool isCache) async {
    parent.target?.chatModel.voiceCallStatus.value = VoiceCallStatus.notStarted;

    _queryAuditGuid(peerId);

    // Map clone is required here, otherwise "evt" may be changed by other threads through the reference.
    // Because this function is asynchronous, there's an "await" in this function.
    cachedPeerData.peerInfo = {...evt};
    // Do not cache resolutions, because a new display connection have different resolutions.
    cachedPeerData.peerInfo.remove('resolutions');

    // Recent peer is updated by handle_peer_info(ui_session_interface.rs) --> handle_peer_info(client.rs) --> save_config(client.rs)
    bind.mainLoadRecentPeers();

    parent.target?.dialogManager.dismissAll();
    _pi.version = evt['version'];
    // Note: Relative mouse mode is NOT auto-enabled on connect.
    // Users must manually enable it via toolbar or keyboard shortcut (Ctrl+Alt+Shift+M).
    //
    // For desktop/webDesktop, keyboard mode initialization is handled later by
    // checkDesktopKeyboardMode() which may change the mode if not supported,
    // followed by updateKeyboardMode() to sync InputModel.keyboardMode.
    // For mobile, updateKeyboardMode() is currently a no-op (only executes on desktop/web),
    // but we call it here for consistency and future-proofing.
    if (isMobile) {
      parent.target?.inputModel.updateKeyboardMode();
    }
    _pi.isSupportMultiUiSession =
        bind.isSupportMultiUiSession(version: _pi.version);
    _pi.username = evt['username'];
    _pi.hostname = evt['hostname'];
    _pi.platform = evt['platform'];
    _pi.sasEnabled = evt['sas_enabled'] == 'true';
    final currentDisplay = int.parse(evt['current_display']);
    if (_pi.primaryDisplay == kInvalidDisplayIndex) {
      _pi.primaryDisplay = currentDisplay;
    }

    if (bind.peerGetSessionsCount(
            id: peerId, connType: parent.target!.connType.index) <=
        1) {
      _pi.currentDisplay = currentDisplay;
    }

    try {
      CurrentDisplayState.find(peerId).value = _pi.currentDisplay;
    } catch (e) {
      //
    }

    final connType = parent.target?.connType;
    if (isPeerAndroid) {
      _touchMode = true;
    } else {
      // `kOptionTouchMode` is originally peer option, but it is moved to local option later.
      // We check local option first, if not set, then check peer option.
      // Because if local option is not empty:
      // 1. User has set the touch mode explicitly.
      // 2. The advanced option (custom client) is set.
      //    Then we choose to use the local option.
      final optLocal = bind.mainGetLocalOption(key: kOptionTouchMode);
      if (optLocal != '') {
        _touchMode = optLocal == 'Y';
      } else {
        final optSession = await bind.sessionGetOption(
            sessionId: sessionId, arg: kOptionTouchMode);
        _touchMode = optSession != '';
      }
    }
    if (isMobile) {
      virtualMouseMode.loadOptions();
    }
    if (connType == ConnType.fileTransfer) {
      parent.target?.fileModel.onReady();
    } else if (connType == ConnType.terminal) {
      // Call onReady on all registered terminal models
      final models = parent.target?._terminalModels.values ?? [];
      for (final model in models) {
        model.onReady();
      }
    } else if (connType == ConnType.defaultConn ||
        connType == ConnType.viewCamera) {
      List<Display> newDisplays = [];
      List<dynamic> displays = json.decode(evt['displays']);
      for (int i = 0; i < displays.length; ++i) {
        newDisplays.add(evtToDisplay(displays[i]));
      }
      _pi.displays.value = newDisplays;
      _pi.displaysCount.value = _pi.displays.length;
      if (_pi.currentDisplay < _pi.displays.length) {
        // now replaced to _updateCurDisplay
        updateCurDisplay(sessionId);
      }
      // After reconnecting, restore the last selected monitor once the canvas is ready.
      // Switching earlier can offset the view if the monitor sizes differ.
      final last = lastUserDisplay;
      pendingMonitorRestore = (!isCache &&
              last != null &&
              last != currentDisplay &&
              bind.sessionGetUseAllMyDisplaysForTheRemoteSession(
                      sessionId: sessionId) !=
                  'Y' &&
              ((last == kAllDisplayValue && _pi.displays.isNotEmpty) ||
                  (last >= 0 && last < _pi.displays.length)))
          ? last
          : null;
      // Fallback if the first image event never reaches this tab (multi-UI).
      _pendingRestoreTimer?.cancel();
      if (pendingMonitorRestore != null) {
        _pendingRestoreTimer = Timer(const Duration(milliseconds: 1500),
            () => parent.target?._applyPendingMonitorRestore());
      }
      if (displays.isNotEmpty) {
        _reconnects = 1;
        _offlineReconnectStartTime = null;
        resetRestartReconnectState();
        waitForFirstImage.value = true;
        isRefreshing = false;
        if (!isCache) fitWindowToPeer(peerId);
      }
      Map<String, dynamic> features = json.decode(evt['features']);
      _pi.features.privacyMode = features['privacy_mode'] == true;
      _pi.features.quickLaunch = features['quick_launch'] == true;
      _pi.features.fileTransferPause = features['file_transfer_pause'] == true;
      if (!isCache) {
        handleResolutions(peerId, evt["resolutions"]);
      }
      parent.target?.elevationModel.onPeerInfo(_pi);
    }
    if (connType == ConnType.defaultConn) {
      setViewOnly(
          peerId,
          bind.sessionGetToggleOptionSync(
              sessionId: sessionId, arg: kOptionToggleViewOnly));
      setShowMyCursor(bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: kOptionToggleShowMyCursor));
    }
    if (connType == ConnType.defaultConn || connType == ConnType.viewCamera) {
      final platformAdditions = evt['platform_additions'];
      if (platformAdditions != null && platformAdditions != '') {
        try {
          _pi.platformAdditions = json.decode(platformAdditions);
        } catch (e) {
          debugPrint('Failed to decode platformAdditions $e');
        }
      }
    }

    _pi.isSet.value = true;
    stateGlobal.resetLastResolutionGroupValues(peerId);

    if (isDesktop) {
      // checkDesktopKeyboardMode may change the keyboard mode if the current
      // mode is not supported. Re-sync InputModel.keyboardMode afterwards.
      // Note: updateKeyboardMode() is a no-op on mobile (early-returns).
      await checkDesktopKeyboardMode();
      await parent.target?.inputModel.updateKeyboardMode();
    }

    _notify();

    if (!isCache) {
      tryUseAllMyDisplaysForTheRemoteSession(peerId);
    }
  }

  checkDesktopKeyboardMode() async {
    if (isInputSourceFlutter) {
      // Local side, flutter keyboard input source
      // Currently only map mode is supported, legacy mode is used for compatibility.
      for (final mode in [kKeyMapMode, kKeyLegacyMode]) {
        if (bind.sessionIsKeyboardModeSupported(
            sessionId: sessionId, mode: mode)) {
          await bind.sessionSetKeyboardMode(sessionId: sessionId, value: mode);
          break;
        }
      }
    } else {
      final curMode = await bind.sessionGetKeyboardMode(sessionId: sessionId);
      if (curMode != null) {
        if (bind.sessionIsKeyboardModeSupported(
            sessionId: sessionId, mode: curMode)) {
          return;
        }
      }

      // If current keyboard mode is not supported, change to another one.
      for (final mode in [kKeyMapMode, kKeyTranslateMode, kKeyLegacyMode]) {
        if (bind.sessionIsKeyboardModeSupported(
            sessionId: sessionId, mode: mode)) {
          bind.sessionSetKeyboardMode(sessionId: sessionId, value: mode);
          break;
        }
      }
    }
  }
}
