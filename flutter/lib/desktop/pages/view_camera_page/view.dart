part of 'view_camera_page.dart';

extension _ViewCameraView on _ViewCameraPageState {
  void enterView(PointerEnterEvent evt) {
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
    if (!isWindows) {
      if (!_rawKeyFocusNode.hasFocus) {
        _rawKeyFocusNode.requestFocus();
      }
      _ffi.inputModel.enterOrLeave(true);
    }
  }

  void leaveView(PointerExitEvent evt) {
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
    if (!isWindows) {
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
      isCamera: true,
    );
  }

  Widget _buildRawPointerMouseRegion(
    Widget child,
    PointerEnterEventListener? onEnter,
    PointerExitEventListener? onExit,
  ) {
    return CameraRawPointerMouseRegion(
      onEnter: onEnter,
      onExit: onExit,
      onPointerDown: (event) {
        // A double check for blur status.
        // Note: If there's an `onPointerDown` event is triggered, `_isWindowBlur` is expected being false.
        // Sometimes the system does not send the necessary focus event to flutter. We should manually
        // handle this inconsistent status by setting `_isWindowBlur` to false. So we can
        // ensure the grab-key thread is running when our users are clicking the remote canvas.
        if (_isWindowBlur) {
          debugPrint(
              "Unexpected status: onPointerDown is triggered while the remote window is in blur status");
          _isWindowBlur = false;
        }
        if (!_rawKeyFocusNode.hasFocus) {
          _rawKeyFocusNode.requestFocus();
        }
      },
      inputModel: _ffi.inputModel,
      child: child,
    );
  }

  Widget getBodyForDesktop(BuildContext context) {
    var paints = <Widget>[
      MouseRegion(onEnter: (evt) {
        if (!isWeb) bind.hostStopSystemKeyPropagate(stopped: false);
      }, onExit: (evt) {
        if (!isWeb) bind.hostStopSystemKeyPropagate(stopped: true);
      }, child: LayoutBuilder(builder: (context, constraints) {
        final c = Provider.of<CanvasModel>(context, listen: false);
        Future.delayed(Duration.zero, () => c.updateViewStyle());
        final peerDisplay = CurrentDisplayState.find(widget.id);
        return Obx(
          () => _ffi.ffiModel.pi.isSet.isFalse
              ? Container(color: Colors.transparent)
              : Obx(() {
                  _ffi.textureModel.updateCurrentDisplay(peerDisplay.value);
                  return ImagePaint(
                    id: widget.id,
                    cursorOverImage: _cursorOverImage,
                    listenerBuilder: (child) => _buildRawTouchAndPointerRegion(
                        child, enterView, leaveView),
                    ffi: _ffi,
                  );
                }),
        );
      }))
    ];

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
