part of 'input_model.dart';

extension InputModelMobile on InputModel {
  /// Web only
  void listenToMouse(bool yesOrNo) {
    if (yesOrNo) {
      platformFFI.startDesktopWebListener();
    } else {
      platformFFI.stopDesktopWebListener();
    }
  }

  void onMobileBack() {
    final minBackButtonVersion = "1.3.8";
    final peerVersion =
        parent.target?.ffiModel.pi.version ?? minBackButtonVersion;
    var btn = MouseButtons.back;
    // For compatibility with old versions
    if (versionCmp(peerVersion, minBackButtonVersion) < 0) {
      btn = MouseButtons.right;
    }
    tap(btn);
  }

  void onMobileHome() => tap(MouseButtons.wheel);
  Future<void> onMobileApps() async {
    sendMouse('down', MouseButtons.wheel);
    await Future.delayed(const Duration(milliseconds: 500));
    sendMouse('up', MouseButtons.wheel);
  }

  // Simulate a key press event.
  // `usbHidUsage` is the USB HID usage code of the key.
  Future<void> tapHidKey(int usbHidUsage) async {
    newKeyboardMode(kKeyFlutterKey, usbHidUsage, true, false);
    await Future.delayed(Duration(milliseconds: 100));
    newKeyboardMode(kKeyFlutterKey, usbHidUsage, false, false);
  }

  Future<void> onMobileVolumeUp() async =>
      await tapHidKey(PhysicalKeyboardKey.audioVolumeUp.usbHidUsage & 0xFFFF);
  Future<void> onMobileVolumeDown() async =>
      await tapHidKey(PhysicalKeyboardKey.audioVolumeDown.usbHidUsage & 0xFFFF);
  Future<void> onMobilePower() async =>
      await tapHidKey(PhysicalKeyboardKey.power.usbHidUsage & 0xFFFF);
}

extension InputModelPointerPos on InputModel {
  Point? _handlePointerDevicePos(
    String kind,
    double x,
    double y,
    bool moveInCanvas,
    CanvasCoords canvas,
    Rect? rect,
    String evtType, {
    bool onExit = false,
    int buttons = kPrimaryMouseButton,
  }) {
    if (rect == null) {
      return null;
    }

    final nearThr = 3;
    var nearRight = (canvas.size.width - x) < nearThr;
    var nearBottom = (canvas.size.height - y) < nearThr;
    final imageWidth = rect.width * canvas.scale;
    final imageHeight = rect.height * canvas.scale;
    if (canvas.scrollStyle != ScrollStyle.scrollauto) {
      x += imageWidth * canvas.scrollX;
      y += imageHeight * canvas.scrollY;

      // boxed size is a center widget
      if (canvas.size.width > imageWidth) {
        x -= ((canvas.size.width - imageWidth) / 2);
      }
      if (canvas.size.height > imageHeight) {
        y -= ((canvas.size.height - imageHeight) / 2);
      }
    } else {
      x -= canvas.x;
      y -= canvas.y;
    }

    x /= canvas.scale;
    y /= canvas.scale;
    if (canvas.scale > 0 && canvas.scale < 1) {
      final step = 1.0 / canvas.scale - 1;
      if (nearRight) {
        x += step;
      }
      if (nearBottom) {
        y += step;
      }
    }
    x += rect.left;
    y += rect.top;

    if (onExit) {
      final pos = setNearestEdge(x, y, rect);
      x = pos.dx;
      y = pos.dy;
    }

    return InputModel.getPointInRemoteRect(
        true, peerPlatform, kind, evtType, x, y, rect,
        buttons: buttons);
  }
}

extension InputModelDevicePos on InputModel {
  Point? handlePointerDevicePos(
    String kind,
    double x,
    double y,
    bool isMove,
    String evtType, {
    bool onExit = false,
    int buttons = kPrimaryMouseButton,
    bool moveCanvas = true,
    bool edgeScroll = false,
  }) {
    final ffiModel = parent.target!.ffiModel;
    CanvasCoords canvas =
        CanvasCoords.fromCanvasModel(parent.target!.canvasModel);
    Rect? rect = ffiModel.rect;

    if (isMove) {
      if (_remoteWindowCoords.isNotEmpty &&
          _windowRect != null &&
          !_isInCurrentWindow(x, y)) {
        final coords =
            InputModel.findRemoteCoords(x, y, _remoteWindowCoords, devicePixelRatio);
        if (coords != null) {
          isMove = false;
          canvas = coords.canvas;
          rect = coords.remoteRect;
          x -= isWindows
              ? coords.relativeOffset.dx / devicePixelRatio
              : coords.relativeOffset.dx;
          y -= isWindows
              ? coords.relativeOffset.dy / devicePixelRatio
              : coords.relativeOffset.dy;
        }
      }
    }

    y -= CanvasModel.topToEdge;
    x -= CanvasModel.leftToEdge;
    if (isMove) {
      final canvasModel = parent.target!.canvasModel;

      if (edgeScroll) {
        canvasModel.edgeScrollMouse(x, y);
      } else if (moveCanvas) {
        canvasModel.moveDesktopMouse(x, y);
      }

      canvasModel.updateLocalCursor(x, y);
    }

    return _handlePointerDevicePos(
      kind,
      x,
      y,
      isMove,
      canvas,
      rect,
      evtType,
      onExit: onExit,
      buttons: buttons,
    );
  }

  bool _isInCurrentWindow(double x, double y) {
    var w = _windowRect!.width;
    var h = _windowRect!.height;
    if (isWindows) {
      w /= devicePixelRatio;
      h /= devicePixelRatio;
    }
    return x >= 0 && y >= 0 && x <= w && y <= h;
  }
}
