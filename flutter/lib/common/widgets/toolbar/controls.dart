part of 'toolbar.dart';

List<TTextMenu> toolbarControls(BuildContext context, String id, FFI ffi) {
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final perms = ffiModel.permissions;
  final sessionId = ffi.sessionId;
  final isDefaultConn = ffi.connType == ConnType.defaultConn;
  final isWaylandPeer = pi.platform == kPeerPlatformLinux && pi.isWayland;

  List<TTextMenu> v = [];
  // elevation
  if (isDefaultConn &&
      perms['keyboard'] != false &&
      ffi.elevationModel.showRequestMenu) {
    v.add(
      TTextMenu(
          child: Text(translate('Request Elevation')),
          onPressed: () =>
              showRequestElevationDialog(sessionId, ffi.dialogManager)),
    );
  }
  // osPassword
  if (isDefaultConn && perms['keyboard'] != false) {
    v.add(
      TTextMenu(
        child: Row(children: [
          Text(translate('OS Password')),
        ]),
        trailingIcon: Transform.scale(
          scale: isDesktop ? 0.8 : 1,
          child: IconButton(
            onPressed: () {
              if (isMobile && Navigator.canPop(context)) {
                Navigator.pop(context);
              }
              handleOsPasswordEditIcon(sessionId, ffi.dialogManager);
            },
            icon: Icon(Icons.edit, color: isMobile ? MyTheme.accent : null),
          ),
        ),
        onPressed: () => handleOsPasswordAction(sessionId, ffi.dialogManager),
      ),
    );
  }
  // paste
  if (isDefaultConn &&
      pi.platform != kPeerPlatformAndroid &&
      perms['keyboard'] != false) {
    v.add(TTextMenu(
        child: Text(translate('Send clipboard keystrokes')),
        onPressed: () async {
          Future<void> sendClipboardKeystrokes() async {
            ClipboardData? data = await Clipboard.getData(Clipboard.kTextPlain);
            if (data != null && data.text != null) {
              bind.sessionInputString(
                  sessionId: sessionId, value: data.text ?? "");
            }
          }

          final allowWaylandKeyboard =
              mainGetPeerBoolOptionSync(id, kPeerOptionAllowWaylandKeyboard);
          if (shouldShowWaylandKeyboardPrompt(
            connectionId: sessionId.toString(),
            isWaylandPeer: isWaylandPeer,
            allowWaylandKeyboardRemembered: allowWaylandKeyboard,
          )) {
            ffi.inputModel.keyboardInputAllowed = false;
            showWaylandKeyboardInputWarningDialog(
              id: id,
              connectionId: sessionId.toString(),
              ffi: ffi,
              onEnable: sendClipboardKeystrokes,
            );
            return;
          }
          await sendClipboardKeystrokes();
        }));
  }
  if (isDefaultConn &&
      isWaylandPeer &&
      (mainGetPeerBoolOptionSync(id, kPeerOptionAllowWaylandKeyboard) ||
          isWaylandKeyboardPromptSuppressedForConnection(
              sessionId.toString()))) {
    v.add(TTextMenu(
        child: Text(translate('wayland-keyboard-input-reset-choice-tip')),
        onPressed: () async {
          var persistedCleared = false;
          try {
            await bind.mainSetPeerOption(
                id: id,
                key: kPeerOptionAllowWaylandKeyboard,
                value: bool2option(kPeerOptionAllowWaylandKeyboard, false));
            persistedCleared = true;
          } catch (e) {
            debugPrint(
                'Failed to clear persisted Wayland keyboard permission: $e');
          } finally {
            clearWaylandKeyboardPromptSuppressedForConnection(
                sessionId.toString());
            ffi.inputModel.keyboardInputAllowed = false;
            if (isMobile) {
              await ffi.invokeMethod("enable_soft_keyboard", false);
            }
          }
          showToast(translate(persistedCleared ? 'Successful' : 'Failed'));
        }));
  }
  // reset canvas
  if (isDefaultConn && isMobile) {
    v.add(TTextMenu(
        child: Text(translate('Reset canvas')),
        onPressed: () => ffi.cursorModel.reset()));
  }

  // https://github.com/rustdesk/rustdesk/pull/9731
  // Does not work for connection established by "accept".
  connectWithToken(
      {bool isFileTransfer = false,
      bool isViewCamera = false,
      bool isTcpTunneling = false,
      bool isTerminal = false}) {
    final connToken = bind.sessionGetConnToken(sessionId: ffi.sessionId);
    connect(context, id,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isTerminal: isTerminal,
        isTcpTunneling: isTcpTunneling,
        connToken: connToken);
  }

  if (isDefaultConn && isDesktop) {
    v.add(
      TTextMenu(
          child: Text(translate('Transfer file')),
          onPressed: () => connectWithToken(isFileTransfer: true)),
    );
    v.add(
      TTextMenu(
          child: Text(translate('View camera')),
          onPressed: () => connectWithToken(isViewCamera: true)),
    );
    v.add(
      TTextMenu(
          child: Text('${translate('Terminal')} (beta)'),
          onPressed: () => connectWithToken(isTerminal: true)),
    );
    v.add(
      TTextMenu(
          child: Text(translate('TCP tunneling')),
          onPressed: () => connectWithToken(isTcpTunneling: true)),
    );
  }
  // note
  if (isDefaultConn && !bind.isDisableAccount()) {
    v.add(
      TTextMenu(
          child: Text(translate('Note')),
          onPressed: () async {
            bool isLogin =
                bind.mainGetLocalOption(key: 'access_token').isNotEmpty;
            if (!isLogin) {
              final res = await loginDialog();
              if (res != true) return;
              // Desktop: send message to main window to refresh login status
              // Web: login is required before connection, so no need to refresh
              // Mobile: same isolate, no need to send message
              if (isDesktop) {
                rustDeskWinManager.call(
                    WindowType.Main, kWindowRefreshCurrentUser, "");
              }
            }
            showAuditDialog(ffi);
          }),
    );
  }
  // divider
  if (isDefaultConn && (isDesktop)) {
    v.add(TTextMenu(child: Offstage(), onPressed: () {}, divider: true));
  }
  // ctrlAltDel
  if (isDefaultConn && !ffiModel.viewOnly && ffiModel.keyboard) {
    // Windows accepts this only from a process with SAS rights, which means
    // the controlled side runs as a service. Leaving the item out there
    // reads as "no such feature" when the truth is "not elevated", so it is
    // shown disabled with the reason instead.
    final canSendSas = pi.platform == kPeerPlatformLinux || pi.sasEnabled;
    final label = Text(translate("Insert Ctrl + Alt + Del"));
    v.add(
      TTextMenu(
          child: canSendSas
              ? label
              : Tooltip(
                  message: translate('ctrl-alt-del-unavailable-tip'),
                  child: label),
          onPressed: canSendSas
              ? () => bind.sessionCtrlAltDel(sessionId: sessionId)
              : null),
    );
  }
  // restart
  if (isDefaultConn &&
      perms['restart'] != false &&
      (pi.platform == kPeerPlatformLinux ||
          pi.platform == kPeerPlatformWindows ||
          pi.platform == kPeerPlatformMacOS)) {
    v.add(
      TTextMenu(
          child: Text(translate('Restart remote device')),
          onPressed: () =>
              showRestartRemoteDevice(pi, id, sessionId, ffi.dialogManager)),
    );
  }
  // insertLock
  if (isDefaultConn && !ffiModel.viewOnly && ffi.ffiModel.keyboard) {
    v.add(
      TTextMenu(
          child: Text(translate('Insert Lock')),
          onPressed: () => bind.sessionLockScreen(sessionId: sessionId)),
    );
  }
  // blockUserInput
  if (isDefaultConn &&
      ffi.ffiModel.keyboard &&
      ffi.ffiModel.permissions['block_input'] != false &&
      pi.platform == kPeerPlatformWindows) // privacy-mode != true ??
  {
    v.add(TTextMenu(
        child: Obx(() => Text(translate(
            '${BlockInputState.find(id).value ? 'Unb' : 'B'}lock user input'))),
        onPressed: () {
          RxBool blockInput = BlockInputState.find(id);
          bind.sessionToggleOption(
              sessionId: sessionId,
              value: '${blockInput.value ? 'un' : ''}block-input');
          blockInput.value = !blockInput.value;
        }));
  }
  // switchSides
  if (isDefaultConn &&
      isDesktop &&
      ffiModel.keyboard &&
      pi.platform != kPeerPlatformAndroid &&
      versionCmp(pi.version, '1.2.0') >= 0 &&
      bind.peerGetSessionsCount(id: id, connType: ffi.connType.index) == 1) {
    v.add(TTextMenu(
        child: Text(translate('Switch Sides')),
        onPressed: () =>
            showConfirmSwitchSidesDialog(sessionId, id, ffi.dialogManager)));
  }
  // refresh
  if (pi.version.isNotEmpty) {
    v.add(TTextMenu(
      child: Text(translate('Refresh')),
      onPressed: () => sessionRefreshVideo(sessionId, pi),
    ));
  }
  // record
  if (!isDesktop &&
      bind.mainGetLocalOption(key: kOptionHideRecordingButton) != 'Y' &&
      (ffi.recordingModel.start || (perms["recording"] != false))) {
    v.add(TTextMenu(
        child: Row(
          children: [
            Text(translate(ffi.recordingModel.start
                ? 'Stop session recording'
                : 'Start session recording')),
            Padding(
              padding: EdgeInsets.only(left: 12),
              child: Icon(
                  ffi.recordingModel.start
                      ? Icons.pause_circle_filled
                      : Icons.videocam_outlined,
                  color: MyTheme.accent),
            )
          ],
        ),
        onPressed: () => ffi.recordingModel.toggle()));
  }

  // to-do:
  // 1. Web desktop
  // 2. Mobile, copy the image to the clipboard
  if ((isDefaultConn || ffi.connType == ConnType.viewCamera) && isDesktop) {
    final isScreenshotSupported = bind.sessionGetCommonSync(
        sessionId: sessionId, key: 'is_screenshot_supported', param: '');
    if ('true' == isScreenshotSupported) {
      v.add(TTextMenu(
        child: Text(ffi.ffiModel.timerScreenshot != null
            ? '${translate('Taking screenshot')} ...'
            : translate('Take screenshot')),
        onPressed: ffi.ffiModel.timerScreenshot != null
            ? null
            : () {
                if (pi.currentDisplay == kAllDisplayValue) {
                  msgBox(
                      sessionId,
                      'custom-nook-nocancel-hasclose-info',
                      'Take screenshot',
                      'screenshot-merged-screen-not-supported-tip',
                      '',
                      ffi.dialogManager);
                } else {
                  bind.sessionTakeScreenshot(
                      sessionId: sessionId, display: pi.currentDisplay);
                  ffi.ffiModel.timerScreenshot =
                      Timer(Duration(seconds: 30), () {
                    ffi.ffiModel.timerScreenshot = null;
                  });
                }
              },
      ));
    }
  }
  // fingerprint
  if (!isDesktop) {
    v.add(TTextMenu(
      child: Text(translate('Copy Fingerprint')),
      onPressed: () => onCopyFingerprint(FingerprintState.find(id).value),
    ));
  }
  return v;
}
