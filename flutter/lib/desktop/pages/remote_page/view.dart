part of 'remote_page.dart';

extension _RemotePageView on _RemotePageState {
  void enterView(PointerEnterEvent evt) {
    _ffi.canvasModel.rearmEdgeScroll();

    _cursorOverImage.value = true;
    _firstEnterImage.value = true;
    if (_onEnterOrLeaveImage4Toolbar != null) {
      try {
        _onEnterOrLeaveImage4Toolbar!(true);
      } catch (e) {
        //
      }
    }

    // See [onWindowBlur].
    if (isMacOS) {
      _macOSLocalFocusLost = false;
      stateGlobal.getInputSource(force: true);
      _syncMacOSKeyboardGrab(reassert: true, allowInactiveLifecycle: true);
    } else if (isWindows) {
      // Blur unfocuses this node and nothing restores it, so the keyboard stayed
      // dead until a click. Focus only while the window is really active, or a
      // background window would grab system keys. onFocusChange does enterOrLeave.
      if (!_isWindowBlur &&
          _windowsCanFocusRemoteInput &&
          !_rawKeyFocusNode.hasFocus) {
        _rawKeyFocusNode.requestFocus();
      }
    } else {
      if (!_rawKeyFocusNode.hasFocus) {
        _rawKeyFocusNode.requestFocus();
      }
      _ffi.inputModel.enterOrLeave(true);
    }
  }

  void leaveView(PointerExitEvent evt) {
    _ffi.canvasModel.disableEdgeScroll();

    if (_ffi.ffiModel.keyboard) {
      _ffi.inputModel.tryMoveEdgeOnExit(evt.position);
    }

    _cursorOverImage.value = false;
    _firstEnterImage.value = false;
    if (_onEnterOrLeaveImage4Toolbar != null) {
      try {
        _onEnterOrLeaveImage4Toolbar!(false);
      } catch (e) {
        //
      }
    }

    // See [onWindowBlur].
    if (isMacOS) {
      _syncMacOSKeyboardGrab();
    } else if (!isWindows) {
      _ffi.inputModel.enterOrLeave(false);
    }
  }

  Widget _buildRawTouchAndPointerRegion(
    Widget child,
    PointerEnterEventListener? onEnter,
    PointerExitEventListener? onExit,
  ) {
    return RawTouchGestureDetectorRegion(
      child: _buildRawPointerMouseRegion(child, onEnter, onExit),
      ffi: _ffi,
    );
  }

  Widget _buildRawPointerMouseRegion(
    Widget child,
    PointerEnterEventListener? onEnter,
    PointerExitEventListener? onExit,
  ) {
    return RawPointerMouseRegion(
      onEnter: onEnter,
      onExit: onExit,
      onPointerDown: (event) {
        // A double check for blur status on Windows and macOS.
        // Note: If there's an `onPointerDown` event is triggered, `_isWindowBlur` is expected being false.
        // Sometimes the system does not send the necessary focus event to flutter. We should manually
        // handle this inconsistent status by setting `_isWindowBlur` to false. So we can
        // ensure the grab-key thread is running when our users are clicking the remote canvas.
        if ((isWindows || isMacOS) && _isWindowBlur) {
          debugPrint(
              "Unexpected status: onPointerDown is triggered while the remote window is in blur status");
          _isWindowBlur = false;
        }
        if (isMacOS) {
          // Regions without matching enter/exit callbacks cannot safely own
          // keyboard state.
          if (onEnter == null || onExit == null) return;
          if (!stateGlobal.isFocused.value) {
            stateGlobal.isFocused.value = true;
          }
          _cursorOverImage.value = true;
          _macOSLocalFocusLost = false;
          stateGlobal.getInputSource(force: true);
          _syncMacOSKeyboardGrab(
              reassert: !isInputSourceFlutter, allowInactiveLifecycle: true);
        } else if (!_rawKeyFocusNode.hasFocus) {
          _rawKeyFocusNode.requestFocus();
        }
      },
      inputModel: _ffi.inputModel,
      child: child,
    );
  }

  Widget getBodyForDesktop(BuildContext context) {
    var paints = <Widget>[
      MouseRegion(
        onEnter: (evt) {
          bind.hostStopSystemKeyPropagate(stopped: false);
        },
        onExit: (evt) {
          bind.hostStopSystemKeyPropagate(stopped: true);
        },
        child: _ViewStyleUpdater(
          canvasModel: _ffi.canvasModel,
          inputModel: _ffi.inputModel,
          child: Builder(builder: (context) {
            final peerDisplay = CurrentDisplayState.find(widget.id);
            return Obx(
              () => _ffi.ffiModel.pi.isSet.isFalse
                  ? Container(color: Colors.transparent)
                  : Obx(() {
                      _ffi.textureModel.updateCurrentDisplay(peerDisplay.value);
                      return ImagePaint(
                        id: widget.id,
                        zoomCursor: _zoomCursor,
                        cursorOverImage: _cursorOverImage,
                        keyboardEnabled: _keyboardEnabled,
                        remoteCursorMoved: _remoteCursorMoved,
                        listenerBuilder: (child) =>
                            _buildRawTouchAndPointerRegion(
                                child, enterView, leaveView),
                        ffi: _ffi,
                      );
                    }),
            );
          }),
        ),
      )
    ];

    if (!_ffi.canvasModel.cursorEmbedded) {
      paints
          .add(Obx(() => _showRemoteCursor.isFalse || _remoteCursorMoved.isFalse
              ? Offstage()
              : CursorPaint(
                  id: widget.id,
                  zoomCursor: _zoomCursor,
                )));
    }
    paints.add(
      Positioned(
        top: 10,
        right: 10,
        child: _buildRawTouchAndPointerRegion(
            QualityMonitor(_ffi.qualityMonitorModel), null, null),
      ),
    );
    return Stack(
      children: paints,
    );
  }
}
