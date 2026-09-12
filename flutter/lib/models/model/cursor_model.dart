part of 'model.dart';

class CursorModel with ChangeNotifier {
  ui.Image? _image;
  final _images = <String, Tuple3<ui.Image, double, double>>{};
  CursorData? _cache;
  final _cacheMap = <String, CursorData>{};
  final _cacheKeys = <String>{};
  double _x = -10000;
  double _y = -10000;
  // int.parse(evt['id']) may cause FormatException
  // So we use String here.
  String _id = "-1";
  double _hotx = 0;
  double _hoty = 0;
  double _displayOriginX = 0;
  double _displayOriginY = 0;
  DateTime? _firstUpdateMouseTime;
  Rect? _windowRect;
  List<RemoteWindowCoords> _remoteWindowCoords = [];
  bool gotMouseControl = true;
  DateTime _lastPeerMouse = DateTime.now()
      .subtract(Duration(milliseconds: 3000 * kMouseControlTimeoutMSec));
  String peerId = '';
  WeakReference<FFI> parent;

  // Only for mobile, touch mode
  // To block touch event above the KeyHelpTools
  //
  // A better way is to not listen events from the KeyHelpTools.
  // But we're now using a Container(child: Stack(...)) to wrap the KeyHelpTools,
  // and the listener is on the Container.
  Rect? _keyHelpToolsRect;
  // `lastIsBlocked` is only used in common/widgets/remote_input.dart -> _RawTouchGestureDetectorRegionState -> onDoubleTap()
  // Because onDoubleTap() doesn't have the `event` parameter, we can't get the touch event's position.
  bool _lastIsBlocked = false;
  bool _lastKeyboardIsVisible = false;

  bool get lastKeyboardIsVisible => _lastKeyboardIsVisible;

  Rect? get keyHelpToolsRectToAdjustCanvas =>
      _lastKeyboardIsVisible ? _keyHelpToolsRect : null;
  // The blocked rect is used to block the pointer/touch events in the remote page.
  final List<Rect> _blockedRects = [];
  // Used in shouldBlock().
  // _blockEvents is a flag to block pointer/touch events on the remote image.
  // It is set to true to prevent accidental touch events in the following scenarios:
  //   1. In floating mouse mode, when the scroll circle is shown.
  //   2. In floating mouse widgets mode, when the left/right buttons are moving.
  //   3. In floating mouse widgets mode, when using the virtual joystick.
  // When _blockEvents is true, all pointer/touch events are blocked regardless of the contents of _blockedRects.
  // _blockedRects contains specific rectangular regions where events are blocked; these are checked when _blockEvents is false.
  // In summary: _blockEvents acts as a global block, while _blockedRects provides fine-grained blocking.
  bool _blockEvents = false;
  List<Rect> get blockedRects => List.unmodifiable(_blockedRects);

  set blockEvents(bool v) => _blockEvents = v;

  keyHelpToolsVisibilityChanged(Rect? rect, bool keyboardIsVisible) {
    _keyHelpToolsRect = rect;
    if (rect == null) {
      _lastIsBlocked = false;
    } else {
      // Block the touch event is safe here.
      // `lastIsBlocked` is only used in onDoubleTap() to block the touch event from the KeyHelpTools.
      // `lastIsBlocked` will be set when the cursor is moving or touch somewhere else.
      _lastIsBlocked = true;
    }
    if (isMobile && _lastKeyboardIsVisible != keyboardIsVisible) {
      if (keyboardIsVisible) {
        parent.target?.canvasModel.saveMobileOffsetBeforeSoftKeyboard();
        parent.target?.canvasModel.mobileFocusCanvasCursor();
        parent.target?.canvasModel.isMobileCanvasChanged = false;
      } else {
        parent.target?.canvasModel.restoreMobileOffsetAfterSoftKeyboard();
      }
    }
    _lastKeyboardIsVisible = keyboardIsVisible;
  }

  addBlockedRect(Rect rect) {
    _blockedRects.add(rect);
  }

  removeBlockedRect(Rect rect) {
    _blockedRects.remove(rect);
  }

  get lastIsBlocked => _lastIsBlocked;

  ui.Image? get image => _image;
  CursorData? get cache => _cache;

  double get x => _x - _displayOriginX;
  double get y => _y - _displayOriginY;

  double get devicePixelRatio => parent.target!.canvasModel.devicePixelRatio;

  Offset get offset => Offset(_x, _y);

  double get hotx => _hotx;
  double get hoty => _hoty;

  set id(String id) => _id = id;

  bool get isPeerControlProtected =>
      DateTime.now().difference(_lastPeerMouse).inMilliseconds <
      kMouseControlTimeoutMSec;

  bool isConnIn2Secs() {
    if (_firstUpdateMouseTime == null) {
      _firstUpdateMouseTime = DateTime.now();
      return true;
    } else {
      return DateTime.now().difference(_firstUpdateMouseTime!).inSeconds < 2;
    }
  }

  CursorModel(this.parent);

  Set<String> get cachedKeys => _cacheKeys;
  addKey(String key) => _cacheKeys.add(key);

  get scale => parent.target?.canvasModel.scale ?? 1.0;

}
