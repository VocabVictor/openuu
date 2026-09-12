part of 'model.dart';

extension FfiStart on FFI {
  /// Start with the given [id]. Only transfer file if [isFileTransfer], only view camera if [isViewCamera], only port forward if [isPortForward].
  void start(
    String id, {
    bool viewOnly = false,
    bool isFileTransfer = false,
    bool isViewCamera = false,
    bool isPortForward = false,
    bool isRdp = false,
    bool isTerminal = false,
    String? switchUuid,
    String? password,
    bool? isSharedPassword,
    String? connToken,
    bool? forceRelay,
    int? tabWindowId,
    int? display,
    List<int>? displays,
  }) async {
    viewOnlySession = viewOnly;
    closed = false;
    if (isMobile) mobileReset();
    assert(
        (!(isPortForward && isViewCamera)) &&
            (!(isViewCamera && isPortForward)) &&
            (!(isPortForward && isFileTransfer)) &&
            (!(isTerminal && isFileTransfer)) &&
            (!(isTerminal && isViewCamera)) &&
            (!(isTerminal && isPortForward)),
        'more than one connect type');
    if (isFileTransfer) {
      connType = ConnType.fileTransfer;
    } else if (isViewCamera) {
      connType = ConnType.viewCamera;
    } else if (isPortForward) {
      connType = ConnType.portForward;
    } else if (isTerminal) {
      connType = ConnType.terminal;
    } else {
      chatModel.resetClientMode();
      connType = ConnType.defaultConn;
      canvasModel.id = id;
      imageModel.id = id;
      cursorModel.peerId = id;
    }

    final isNewPeer = tabWindowId == null;
    // If tabWindowId != null, this session is a "tab -> window" one.
    // Else this session is a new one.
    if (isNewPeer) {
      // ignore: unused_local_variable
      final addRes = bind.sessionAddSync(
        sessionId: sessionId,
        id: id,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isPortForward: isPortForward,
        isRdp: isRdp,
        isTerminal: isTerminal,
        switchUuid: switchUuid ?? '',
        forceRelay: forceRelay ?? false,
        password: password ?? '',
        isSharedPassword: isSharedPassword ?? false,
        connToken: connToken,
      );
      if (viewOnly && addRes != '') {
        msgBox(sessionId, 'error', 'View Mode', addRes, '', dialogManager);
        return;
      }
      if (viewOnly) {
        await bind.sessionPeerOption(sessionId: sessionId,
            name: 'view-only-session', value: 'Y');
        final supported = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: 'view-only-session');
        if (closed) return;
        if (supported != true) {
          bind.sessionClose(sessionId: sessionId);
          msgBox(sessionId, 'error', 'View Mode',
              'Cannot start a locked view-only session. Update the native library or close the existing control session first.', '', dialogManager);
          return;
        }
        ffiModel.setViewOnly(id, true);
      }
    } else if (display != null) {
      if (displays == null) {
        debugPrint(
            'Unreachable, failed to add existed session to $id, the displays is null while display is $display');
        return;
      }
      final addRes = bind.sessionAddExistedSync(
          id: id,
          sessionId: sessionId,
          displays: Int32List.fromList(displays),
          isViewCamera: isViewCamera);
      if (addRes != '') {
        debugPrint(
            'Unreachable, failed to add existed session to $id, $addRes');
        return;
      }
      ffiModel.pi.currentDisplay = display;
    }
    if (!isNewPeer) {
      viewOnlySession = await bind.sessionGetToggleOption(
          sessionId: sessionId, arg: 'view-only-session') == true;
      if (closed) return;
    }
    if (isDesktop && connType == ConnType.defaultConn) {
      textureModel.updateCurrentDisplay(display ?? 0);
    }
    // FIXME: separate cameras displays or shift all indices.
    if (isDesktop && connType == ConnType.viewCamera) {
      // FIXME: currently the default 0 is not used.
      textureModel.updateCurrentDisplay(display ?? 0);
    }

    if (isDesktop) {
      inputModel.updateTrackpadSpeed();
    }

    // CAUTION: `sessionStart()` and `sessionStartWithDisplays()` are an async functions.
    // Though the stream is returned immediately, the stream may not be ready.
    // Any operations that depend on the stream should be carefully handled.
    late final Stream<EventToUI> stream;
    if (isNewPeer || display == null || displays == null) {
      stream = bind.sessionStart(sessionId: sessionId, id: id);
    } else {
      // We have to put displays in `sessionStart()` to make sure the stream is ready
      // and then the displays' capturing requests can be sent.
      stream = bind.sessionStartWithDisplays(
          sessionId: sessionId, id: id, displays: Int32List.fromList(displays));
    }

    if (isWeb) {
      platformFFI.setRgbaCallback((int display, Uint8List data) {
        onEvent2UIRgba();
        imageModel.onRgba(display, data);
      });
      platformFFI.setVideoFrameCallback((int display, ui.Image image,
          bool Function() isCurrentSession) async {
        if (!isCurrentSession()) {
          image.dispose();
          return;
        }
        await onEvent2UIRgba();
        await imageModel.onImage(display, image, isCurrentSession);
      });
      this.id = id;
      return;
    }

    final cb = ffiModel.startEventListener(sessionId, id);

    imageModel.updateUserTextureRender();
    final hasGpuTextureRender = bind.mainHasGpuTextureRender();
    final SimpleWrapper<bool> isToNewWindowNotified = SimpleWrapper(false);
    // Preserved for the rgba data.
    stream.listen((message) {
      if (closed) return;
      if (tabWindowId != null && !isToNewWindowNotified.value) {
        // Session is read to be moved to a new window.
        // Get the cached data and handle the cached data.
        Future.delayed(Duration.zero, () async {
          final args = jsonEncode({'id': id, 'close': display == null});
          final cachedData = await DesktopMultiWindow.invokeMethod(
              tabWindowId, kWindowEventGetCachedSessionData, args);
          if (cachedData == null) {
            // unreachable
            debugPrint('Unreachable, the cached data is empty.');
            return;
          }
          final data = CachedPeerData.fromString(cachedData);
          if (data == null) {
            debugPrint('Unreachable, the cached data cannot be decoded.');
            return;
          }
          ffiModel.setPermissions(data.permissions);
          await ffiModel.handleCachedPeerData(data, id);
          await sessionRefreshVideo(sessionId, ffiModel.pi);
          await bind.sessionRequestNewDisplayInitMsgs(
              sessionId: sessionId, display: ffiModel.pi.currentDisplay);
        });
        isToNewWindowNotified.value = true;
      }
      () async {
        if (message is EventToUI_Event) {
          if (message.field0 == "close") {
            closed = true;
            debugPrint('Exit session event loop');
            return;
          }

          Map<String, dynamic>? event;
          try {
            event = json.decode(message.field0);
          } catch (e) {
            debugPrint('json.decode fail1(): $e, ${message.field0}');
          }
          if (event != null) {
            await cb(event);
          }
        } else if (message is EventToUI_Rgba) {
          final display = message.field0;
          // Fetch the image buffer from rust codes.
          final sz = platformFFI.getRgbaSize(sessionId, display);
          if (sz == 0) {
            platformFFI.nextRgba(sessionId, display);
            return;
          }
          final rgba = platformFFI.getRgba(sessionId, display, sz);
          if (rgba != null) {
            onEvent2UIRgba();
            await imageModel.onRgba(display, rgba);
          } else {
            platformFFI.nextRgba(sessionId, display);
          }
        } else if (message is EventToUI_Texture) {
          final display = message.field0;
          final gpuTexture = message.field1;
          debugPrint(
              "EventToUI_Texture display:$display, gpuTexture:$gpuTexture");
          if (gpuTexture && !hasGpuTextureRender) {
            debugPrint('the gpuTexture is not supported.');
            return;
          }
          textureModel.setTextureType(display: display, gpuTexture: gpuTexture);
          onEvent2UIRgba();
        }
      }();
    });
    // every instance will bind a stream
    this.id = id;
  }
}
