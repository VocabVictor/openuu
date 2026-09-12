part of 'model.dart';

extension FfiModelSync on FfiModel {
  /// Handle the peer info synchronization event based on [evt].
  handleSyncPeerInfo(
      Map<String, dynamic> evt, SessionID sessionId, String peerId) async {
    if (evt['displays'] != null) {
      cachedPeerData.peerInfo['displays'] = evt['displays'];
      List<dynamic> displays = json.decode(evt['displays']);
      List<Display> newDisplays = [];
      for (int i = 0; i < displays.length; ++i) {
        newDisplays.add(evtToDisplay(displays[i]));
      }
      _pi.displays.value = newDisplays;
      _pi.displaysCount.value = _pi.displays.length;

      if (_pi.currentDisplay == kAllDisplayValue) {
        updateCurDisplay(sessionId);
        // to-do: What if the displays are changed?
      } else {
        if (_pi.currentDisplay >= 0 &&
            _pi.currentDisplay < _pi.displays.length) {
          updateCurDisplay(sessionId);
        } else {
          if (_pi.displays.isNotEmpty) {
            // Notify to switch display
            msgBox(sessionId, 'custom-nook-nocancel-hasclose-info', 'Prompt',
                'display_is_plugged_out_msg', '', parent.target!.dialogManager);
            final isPeerPrimaryDisplayValid =
                this.pi.primaryDisplay == kInvalidDisplayIndex ||
                    this.pi.primaryDisplay >= this.pi.displays.length;
            final newDisplay =
                isPeerPrimaryDisplayValid ? 0 : this.pi.primaryDisplay;
            bind.sessionSwitchDisplay(
              isDesktop: isDesktop,
              sessionId: sessionId,
              value: Int32List.fromList([newDisplay]),
            );

            if (_pi.isSupportMultiUiSession) {
              // If the peer supports multi-ui-session, no switch display message will be send back.
              // We need to update the display manually.
              switchToNewDisplay(newDisplay, sessionId, peerId);
            }
          } else {
            msgBox(sessionId, 'nocancel-error', 'Prompt', 'No Displays', '',
                parent.target!.dialogManager);
          }
        }
      }
    }
    parent.target!.canvasModel
        .tryUpdateScrollStyle(Duration(milliseconds: 300), null);
    _notify();
  }

  handlePlatformAdditions(
      Map<String, dynamic> evt, SessionID sessionId, String peerId) async {
    final updateData = evt['platform_additions'] as String?;
    if (updateData == null) {
      return;
    }

    if (updateData.isEmpty) {
      _pi.platformAdditions.remove(kPlatformAdditionsRustDeskVirtualDisplays);
      _pi.platformAdditions.remove(kPlatformAdditionsAmyuniVirtualDisplays);
    } else {
      try {
        final updateJson = json.decode(updateData) as Map<String, dynamic>;
        for (final key in updateJson.keys) {
          _pi.platformAdditions[key] = updateJson[key];
        }
        if (!updateJson
            .containsKey(kPlatformAdditionsRustDeskVirtualDisplays)) {
          _pi.platformAdditions
              .remove(kPlatformAdditionsRustDeskVirtualDisplays);
        }
        if (!updateJson.containsKey(kPlatformAdditionsAmyuniVirtualDisplays)) {
          _pi.platformAdditions.remove(kPlatformAdditionsAmyuniVirtualDisplays);
        }
      } catch (e) {
        debugPrint('Failed to decode platformAdditions $e');
      }
    }

    cachedPeerData.peerInfo['platform_additions'] =
        json.encode(_pi.platformAdditions);
  }

  handleFollowCurrentDisplay(
      Map<String, dynamic> evt, SessionID sessionId, String peerId) async {
    if (evt['display_idx'] != null) {
      if (this.pi.currentDisplay == kAllDisplayValue) {
        return;
      }
      _pi.currentDisplay = int.parse(evt['display_idx']);
      try {
        CurrentDisplayState.find(peerId).value = _pi.currentDisplay;
      } catch (e) {
        //
      }
      bind.sessionSwitchDisplay(
        isDesktop: isDesktop,
        sessionId: sessionId,
        value: Int32List.fromList([_pi.currentDisplay]),
      );
    }
    _notify();
  }

  // Directly switch to the new display without waiting for the response.
  switchToNewDisplay(int display, SessionID sessionId, String peerId,
      {bool updateCursorPos = false}) {
    // no need to wait for the response
    this.pi.currentDisplay = display;
    updateCurDisplay(sessionId, updateCursorPos: updateCursorPos);
    try {
      CurrentDisplayState.find(peerId).value = display;
    } catch (e) {
      //
    }
  }

  updateBlockInputState(Map<String, dynamic> evt, String peerId) {
    _inputBlocked = evt['input_state'] == 'on';
    _notify();
    try {
      BlockInputState.find(peerId).value = evt['input_state'] == 'on';
    } catch (e) {
      //
    }
  }

  updatePrivacyMode(
      Map<String, dynamic> evt, SessionID sessionId, String peerId) async {
    _notify();
    try {
      final isOn = bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: 'privacy-mode');
      if (isOn) {
        var privacyModeImpl = await bind.sessionGetOption(
            sessionId: sessionId, arg: 'privacy-mode-impl-key');
        // For compatibility, version < 1.2.4, the default value is 'privacy_mode_impl_mag'.
        final initDefaultPrivacyMode = 'privacy_mode_impl_mag';
        PrivacyModeState.find(peerId).value =
            privacyModeImpl ?? initDefaultPrivacyMode;
      } else {
        PrivacyModeState.find(peerId).value = '';
      }
    } catch (e) {
      //
    }
  }

  void setViewOnly(String id, bool value) {
    if (versionCmp(_pi.version, '1.2.0') < 0) return;
    // tmp fix for https://github.com/rustdesk/rustdesk/pull/3706#issuecomment-1481242389
    // because below rx not used in mobile version, so not initialized, below code will cause crash
    // current our flutter code quality is fucking shit now. !!!!!!!!!!!!!!!!
    try {
      if (value) {
        ShowRemoteCursorState.find(id).value = value;
      } else {
        ShowRemoteCursorState.find(id).value = bind.sessionGetToggleOptionSync(
            sessionId: sessionId, arg: 'show-remote-cursor');
      }
    } catch (e) {
      //
    }
    if (_viewOnly != value) {
      _viewOnly = value;
      _notify();
    }
  }

  void setShowMyCursor(bool value) {
    if (_showMyCursor != value) {
      _showMyCursor = value;
      _notify();
    }
  }
}
