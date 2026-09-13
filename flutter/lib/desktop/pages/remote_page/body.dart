part of 'remote_page.dart';

extension _RemotePageBody on _RemotePageState {
  Widget emptyOverlay() => BlockableOverlay(
        /// the Overlay key will be set with _blockableOverlayState in BlockableOverlay
        /// see override build() in [BlockableOverlay]
        state: _blockableOverlayState,
        underlying: Container(
          color: Colors.transparent,
        ),
      );

  Widget buildBody(BuildContext context) {
    remoteToolbar(BuildContext context) => RemoteToolbar(
          id: widget.id,
          ffi: _ffi,
          state: widget.toolbarState,
          onEnterOrLeaveImageSetter: (id, func) {
            _instanceIdOnEnterOrLeaveImage4Toolbar = id;
            _onEnterOrLeaveImage4Toolbar = func;
          },
          onEnterOrLeaveImageCleaner: (id) {
            // If _instanceIdOnEnterOrLeaveImage4Toolbar != id
            // it means `_onEnterOrLeaveImage4Toolbar` is not set or it has been changed to another toolbar.
            if (_instanceIdOnEnterOrLeaveImage4Toolbar == id) {
              _instanceIdOnEnterOrLeaveImage4Toolbar = null;
              _onEnterOrLeaveImage4Toolbar = null;
            }
          },
          setRemoteState: _setState,
        );

    bodyWidget() {
      return Stack(
        children: [
          Container(
              color: kColorCanvas,
              child: RawKeyFocusScope(
                  focusNode: _rawKeyFocusNode,
                  onFocusChange: (bool imageFocused) {
                    debugPrint(
                        "onFocusChange(window active:${!_isWindowBlur}) $imageFocused");
                    // See [onWindowBlur].
                    if (isWindows) {
                      if (_isWindowBlur) {
                        imageFocused = false;
                        Future.delayed(Duration.zero, () {
                          _rawKeyFocusNode.unfocus();
                        });
                      }
                      if (imageFocused) {
                        _ffi.inputModel.enterOrLeave(true);
                      } else {
                        _ffi.inputModel.enterOrLeave(false);
                      }
                    } else if (isMacOS) {
                      _onMacOSFocusChange();
                    }
                  },
                  inputModel: _ffi.inputModel,
                  child: getBodyForDesktop(context))),
          Stack(
            children: [
              _ffi.ffiModel.pi.isSet.isTrue &&
                      _ffi.ffiModel.waitForFirstImage.isTrue
                  ? emptyOverlay()
                  : () {
                      if (!_ffi.ffiModel.isPeerAndroid) {
                        return Offstage();
                      } else {
                        return Obx(() => Offstage(
                              offstage: _ffi.dialogManager
                                  .mobileActionsOverlayVisible.isFalse,
                              child: Overlay(initialEntries: [
                                makeMobileActionsOverlayEntry(
                                  () => _ffi.dialogManager
                                      .setMobileActionsOverlayVisible(false),
                                  ffi: _ffi,
                                )
                              ]),
                            ));
                      }
                    }(),
              // Use Overlay to enable rebuild every time on menu button click.
              // Hide toolbar when relative mouse mode is active to prevent
              // cursor from escaping to toolbar area.
              Obx(() => _ffi.inputModel.relativeMouseMode.value
                  ? const Offstage()
                  : _ffi.ffiModel.pi.isSet.isTrue
                      ? Overlay(initialEntries: [
                          OverlayEntry(builder: remoteToolbar)
                        ])
                      : remoteToolbar(context)),
              _ffi.ffiModel.pi.isSet.isFalse ? emptyOverlay() : Offstage(),
              SessionStatusBar(
                controller: _statusController,
                onReconnect: () => _ffi.ffiModel
                    .reconnect(_ffi.dialogManager, sessionId, false),
                onDisconnect: closeConnection,
              ),
            ],
          ),
        ],
      );
    }

    return Scaffold(
      backgroundColor: Theme.of(context).colorScheme.background,
      body: Obx(() {
        final imageReady = _ffi.ffiModel.pi.isSet.isTrue &&
            _ffi.ffiModel.waitForFirstImage.isFalse;
        if (imageReady) {
          // If the privacy mode(disable physical displays) is switched,
          // we should not dismiss the dialog immediately.
          if (DateTime.now().difference(togglePrivacyModeTime) >
              const Duration(milliseconds: 3000)) {
            // `dismissAll()` is to ensure that the state is clean.
            // It's ok to call dismissAll() here.
            _ffi.dialogManager.dismissAll();
            // Recreate the block state to refresh the state.
            _blockableOverlayState = BlockableOverlayState();
            _blockableOverlayState.applyFfi(_ffi);
          }
          // Block the whole `bodyWidget()` when dialog shows.
          return BlockableOverlay(
            underlying: bodyWidget(),
            state: _blockableOverlayState,
          );
        } else {
          // `_blockableOverlayState` is not recreated here.
          // The toolbar's block state won't work properly when reconnecting, but that's okay.
          return bodyWidget();
        }
      }),
    );
  }
}
