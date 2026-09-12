part of 'model.dart';

class VirtualMouseMode with ChangeNotifier {
  bool _showVirtualMouse = false;
  double _virtualMouseScale = 1.0;
  bool _showVirtualJoystick = false;

  bool get showVirtualMouse => _showVirtualMouse;
  double get virtualMouseScale => _virtualMouseScale;
  bool get showVirtualJoystick => _showVirtualJoystick;

  FfiModel ffiModel;

  VirtualMouseMode(this.ffiModel);

  bool _shouldShow() => !ffiModel.isPeerAndroid;

  setShowVirtualMouse(bool b) {
    if (b == _showVirtualMouse) return;
    if (_shouldShow()) {
      _showVirtualMouse = b;
      notifyListeners();
    }
  }

  setVirtualMouseScale(double s) {
    if (s <= 0) return;
    if (s == _virtualMouseScale) return;
    _virtualMouseScale = s;
    bind.mainSetLocalOption(key: kOptionVirtualMouseScale, value: s.toString());
    notifyListeners();
  }

  setShowVirtualJoystick(bool b) {
    if (b == _showVirtualJoystick) return;
    if (_shouldShow()) {
      _showVirtualJoystick = b;
      notifyListeners();
    }
  }

  void loadOptions() {
    _showVirtualMouse =
        bind.mainGetLocalOption(key: kOptionShowVirtualMouse) == 'Y';
    _virtualMouseScale = double.tryParse(
            bind.mainGetLocalOption(key: kOptionVirtualMouseScale)) ??
        1.0;
    _showVirtualJoystick =
        bind.mainGetLocalOption(key: kOptionShowVirtualJoystick) == 'Y';
    notifyListeners();
  }

  Future<void> toggleVirtualMouse() async {
    await bind.mainSetLocalOption(
        key: kOptionShowVirtualMouse, value: showVirtualMouse ? 'N' : 'Y');
    setShowVirtualMouse(
        bind.mainGetLocalOption(key: kOptionShowVirtualMouse) == 'Y');
  }

  Future<void> toggleVirtualJoystick() async {
    await bind.mainSetLocalOption(
        key: kOptionShowVirtualJoystick,
        value: showVirtualJoystick ? 'N' : 'Y');
    setShowVirtualJoystick(
        bind.mainGetLocalOption(key: kOptionShowVirtualJoystick) == 'Y');
  }
}

class ImageModel with ChangeNotifier {
  ui.Image? _image;

  ui.Image? get image => _image;

  String id = '';

  late final SessionID sessionId;

  bool _useTextureRender = false;

  WeakReference<FFI> parent;

  final List<Function(String)> callbacksOnFirstImage = [];

  ImageModel(this.parent) {
    sessionId = parent.target!.sessionId;
  }

  get useTextureRender => _useTextureRender;

  addCallbackOnFirstImage(Function(String) cb) => callbacksOnFirstImage.add(cb);

  clearImage() => _image = null;

  bool _webDecodingRgba = false;
  final List<Uint8List> _webRgbaList = List.empty(growable: true);
  webOnRgba(int display, Uint8List rgba) async {
    // deep copy needed, otherwise "instantiateCodec failed: TypeError: Cannot perform Construct on a detached ArrayBuffer"
    _webRgbaList.add(Uint8List.fromList(rgba));
    if (_webDecodingRgba) {
      return;
    }
    _webDecodingRgba = true;
    try {
      while (_webRgbaList.isNotEmpty) {
        final rgba2 = _webRgbaList.last;
        _webRgbaList.clear();
        await decodeAndUpdate(display, rgba2);
      }
    } catch (e) {
      debugPrint('onRgba error: $e');
    }
    _webDecodingRgba = false;
  }

  onRgba(int display, Uint8List rgba) async {
    try {
      await decodeAndUpdate(display, rgba);
    } catch (e) {
      debugPrint('onRgba error: $e');
    }
    platformFFI.nextRgba(sessionId, display);
  }

  // web only: image already created from a decoded WebCodecs frame
  Future<void> onImage(
      int display, ui.Image image, bool Function() isCurrentSession) async {
    await update(image, isCurrentSession: isCurrentSession);
  }

  decodeAndUpdate(int display, Uint8List rgba) async {
    final pid = parent.target?.id;
    final rect = parent.target?.ffiModel.pi.getDisplayRect(display);
    final image = await img.decodeImageFromPixels(
      rgba,
      rect?.width.toInt() ?? 0,
      rect?.height.toInt() ?? 0,
      isWeb | isWindows | isLinux
          ? ui.PixelFormat.rgba8888
          : ui.PixelFormat.bgra8888,
    );
    if (parent.target?.id != pid) {
      image?.dispose();
      return;
    }
    await update(image);
  }

  Future<void> update(ui.Image? image,
      {bool Function()? isCurrentSession}) async {
    if (_disposeIfStale(image, isCurrentSession)) return;
    if (_image == null && image != null) {
      if (isDesktop || isWebDesktop) {
        await parent.target?.canvasModel.updateViewStyle();
        await parent.target?.canvasModel.updateScrollStyle();
        await parent.target?.canvasModel.initializeEdgeScrollEdgeThickness();
      }
      if (parent.target != null) {
        await initializeCursorAndCanvas(parent.target!);
      }
    }
    if (_disposeIfStale(image, isCurrentSession)) return;
    _image?.dispose();
    _image = image;
    if (image != null) notifyListeners();
  }

  bool _disposeIfStale(ui.Image? image, bool Function()? isCurrentSession) {
    if (image == null || isCurrentSession == null) return false;
    if (isCurrentSession()) return false;
    image.dispose();
    return true;
  }

  // mobile only
  double get maxScale {
    if (_image == null) return 1.5;
    final size = parent.target!.canvasModel.getSize();
    final xscale = size.width / _image!.width;
    final yscale = size.height / _image!.height;
    return max(1.5, max(xscale, yscale));
  }

  // mobile only
  double get minScale {
    if (_image == null) return 1.5;
    final size = parent.target!.canvasModel.getSize();
    final xscale = size.width / _image!.width;
    final yscale = size.height / _image!.height;
    return min(xscale, yscale) / 1.5;
  }

  updateUserTextureRender() {
    final preValue = _useTextureRender;
    _useTextureRender = isDesktop && bind.mainGetUseTextureRender();
    if (preValue != _useTextureRender) {
      notifyListeners();
    }
  }

  setUseTextureRender(bool value) {
    _useTextureRender = value;
    notifyListeners();
  }

  void disposeImage() {
    _image?.dispose();
    _image = null;
  }
}
