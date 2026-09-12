part of 'model.dart';

extension FfiModelListener on FfiModel {
  // todo: why called by two position
  StreamEventHandler startEventListener(SessionID sessionId, String peerId) {
    return (evt) async {
      var name = evt['name'];
      if (name == 'quick_launch_response') {
        QuickLaunchRequests.receive(evt['response'] ?? '', scope: sessionId.toString());
      } else if (name == 'msgbox') {
        handleMsgBox(evt, sessionId, peerId);
      } else if (name == 'toast') {
        handleToast(evt, sessionId, peerId);
      } else if (name == 'set_multiple_windows_session') {
        handleMultipleWindowsSession(evt, sessionId, peerId);
      } else if (name == 'peer_info') {
        handlePeerInfo(evt, peerId, false);
      } else if (name == 'sync_peer_info') {
        handleSyncPeerInfo(evt, sessionId, peerId);
      } else if (name == 'sync_platform_additions') {
        handlePlatformAdditions(evt, sessionId, peerId);
      } else if (name == 'connection_ready') {
        setConnectionType(peerId, evt['secure'] == 'true',
            evt['direct'] == 'true', evt['stream_type'] ?? '');
        resetRestartReconnectState();
      } else if (name == 'switch_display') {
        // switch display is kept for backward compatibility
        handleSwitchDisplay(evt, sessionId, peerId);
      } else if (name == 'cursor_data') {
        updateLastCursorId(evt);
        await handleCursorData(evt);
      } else if (name == 'cursor_id') {
        updateLastCursorId(evt);
        handleCursorId(evt);
      } else if (name == 'cursor_position') {
        await parent.target?.cursorModel.updateCursorPosition(evt, peerId);
      } else if (name == 'clipboard') {
        Clipboard.setData(ClipboardData(text: evt['content']));
      } else if (name == 'permission') {
        updatePermission(evt, peerId);
      } else if (name == 'chat_client_mode') {
        parent.target?.chatModel
            .receive(ChatModel.clientModeID, evt['text'] ?? '');
      } else if (name == 'chat_server_mode') {
        parent.target?.chatModel
            .receive(int.parse(evt['id'] as String), evt['text'] ?? '');
      } else if (name == 'terminal_response') {
        parent.target?.routeTerminalResponse(evt);
      } else if (name == 'file_dir') {
        parent.target?.fileModel.receiveFileDir(evt);
      } else if (name == 'empty_dirs') {
        parent.target?.fileModel.receiveEmptyDirs(evt);
      } else if (name == 'job_progress') {
        parent.target?.fileModel.jobController.tryUpdateJobProgress(evt);
      } else if (name == 'job_done') {
        bool? refresh =
            await parent.target?.fileModel.jobController.jobDone(evt);
        if (refresh == true) {
          // many job done for delete directory
          // todo: refresh may not work when confirm delete local directory
          parent.target?.fileModel.refreshAll();
        }
      } else if (name == 'job_error') {
        parent.target?.fileModel.handleJobError(evt);
      } else if (name == 'override_file_confirm') {
        parent.target?.fileModel.postOverrideFileConfirm(evt);
      } else if (name == 'load_last_job') {
        parent.target?.fileModel.jobController.loadLastJob(evt);
      } else if (name == 'update_folder_files') {
        parent.target?.fileModel.jobController.updateFolderFiles(evt);
      } else if (name == 'add_connection') {
        parent.target?.serverModel.addConnection(evt);
      } else if (name == 'on_client_remove') {
        parent.target?.serverModel.onClientRemove(evt);
      } else if (name == 'update_quality_status') {
        parent.target?.qualityMonitorModel.updateQualityStatus(evt);
      } else if (name == 'update_block_input_state') {
        updateBlockInputState(evt, peerId);
      } else if (name == 'update_privacy_mode') {
        updatePrivacyMode(evt, sessionId, peerId);
      } else if (name == 'show_elevation') {
        final show = evt['show'].toString() == 'true';
        parent.target?.serverModel.setShowElevation(show);
      } else if (name == 'cancel_msgbox') {
        cancelMsgBox(evt, sessionId);
      } else if (name == 'switch_back') {
        final peer_id = evt['peer_id'].toString();
        await bind.sessionSwitchSides(sessionId: sessionId);
        closeConnection(id: peer_id);
      } else if (name == 'portable_service_running') {
        _handlePortableServiceRunning(peerId, evt);
      } else if (name == 'on_url_scheme_received') {
        // currently comes from "_url" ipc of mac and dbus of linux
        onUrlSchemeReceived(evt);
      } else if (name == 'on_voice_call_waiting') {
        // Waiting for the response from the peer.
        parent.target?.chatModel.onVoiceCallWaiting();
      } else if (name == 'on_voice_call_started') {
        // Voice call is connected.
        parent.target?.chatModel.onVoiceCallStarted();
      } else if (name == 'on_voice_call_closed') {
        // Voice call is closed with reason.
        final reason = evt['reason'].toString();
        parent.target?.chatModel.onVoiceCallClosed(reason);
      } else if (name == 'on_voice_call_incoming') {
        // Voice call is requested by the peer.
        parent.target?.chatModel.onVoiceCallIncoming();
      } else if (name == 'update_voice_call_state') {
        parent.target?.serverModel.updateVoiceCallState(evt);
      } else if (name == 'fingerprint') {
        FingerprintState.find(peerId).value = evt['fingerprint'] ?? '';
      } else if (name == "sync_peer_hash_password_to_personal_ab") {
        if (desktopType == DesktopType.main || isWeb || isMobile) {
          final id = evt['id'];
          final hash = evt['hash'];
          if (id != null && hash != null) {
            gFFI.abModel
                .changePersonalHashPassword(id.toString(), hash.toString());
          }
        }
      } else if (name == "cm_file_transfer_log") {
        if (isDesktop) {
          gFFI.cmFileModel.onFileTransferLog(evt);
        }
      } else if (name == 'sync_peer_option') {
        _handleSyncPeerOption(evt, peerId);
      } else if (name == 'follow_current_display') {
        handleFollowCurrentDisplay(evt, sessionId, peerId);
      } else if (name == 'use_texture_render') {
        _handleUseTextureRender(evt, sessionId, peerId);
      } else if (name == "selected_files") {
        if (isWeb) {
          parent.target?.fileModel.onSelectedFiles(evt);
        }
      } else if (name == "send_emptry_dirs") {
        if (isWeb) {
          parent.target?.fileModel.sendEmptyDirs(evt);
        }
      } else if (name == "record_status") {
        if (desktopType == DesktopType.remote ||
            desktopType == DesktopType.viewCamera ||
            isMobile) {
          parent.target?.recordingModel.updateStatus(evt['start'] == 'true');
        }
      } else if (name == 'screenshot') {
        _handleScreenshot(evt, sessionId, peerId);
      } else if (name == 'exit_relative_mouse_mode') {
        // Handle exit shortcut from rdev grab loop (Ctrl+Alt on Win/Linux, Cmd+G on macOS)
        parent.target?.inputModel.exitRelativeMouseModeWithKeyRelease();
      } else {
        debugPrint('Event is not handled in the fixed branch: $name');
      }
    };
  }

  _handleScreenshot(
      Map<String, dynamic> evt, SessionID sessionId, String peerId) {
    timerScreenshot?.cancel();
    timerScreenshot = null;
    final msg = evt['msg'] ?? '';
    final msgBoxType = 'custom-nook-nocancel-hasclose';
    final msgBoxTitle = 'Take screenshot';
    final dialogManager = parent.target!.dialogManager;
    if (msg.isNotEmpty) {
      msgBox(sessionId, msgBoxType, msgBoxTitle, msg, '', dialogManager);
    } else {
      final msgBoxText = 'screenshot-action-tip';

      close() {
        dialogManager.dismissAll();
      }

      saveAs() {
        close();
        Future.delayed(Duration.zero, () async {
          final ts = DateTime.now().millisecondsSinceEpoch ~/ 1000;
          String? outputFile = await FilePicker.platform.saveFile(
            dialogTitle: '${translate('Save as')}...',
            fileName: 'screenshot_$ts.png',
            allowedExtensions: ['png'],
            type: FileType.custom,
          );
          if (outputFile == null) {
            bind.sessionHandleScreenshot(sessionId: sessionId, action: '2');
          } else {
            final res = await bind.sessionHandleScreenshot(
                sessionId: sessionId, action: '0:$outputFile');
            if (res.isNotEmpty) {
              msgBox(sessionId, 'custom-nook-nocancel-hasclose-error',
                  'Take screenshot', res, '', dialogManager);
            }
          }
        });
      }

      copyToClipboard() {
        bind.sessionHandleScreenshot(sessionId: sessionId, action: '1');
        close();
      }

      cancel() {
        bind.sessionHandleScreenshot(sessionId: sessionId, action: '2');
        close();
      }

      final List<Widget> buttons = [
        dialogButton('${translate('Save as')}...', onPressed: saveAs),
        dialogButton('Copy to clipboard', onPressed: copyToClipboard),
        dialogButton('Cancel', onPressed: cancel),
      ];
      dialogManager.dismissAll();
      dialogManager.show(
        (setState, close, context) => CustomAlertDialog(
          title: null,
          content: SelectionArea(
              child: msgboxContent(msgBoxType, msgBoxTitle, msgBoxText)),
          actions: buttons,
        ),
        tag: '$msgBoxType-$msgBoxTitle-$msgBoxTitle',
      );
    }
  }
}
