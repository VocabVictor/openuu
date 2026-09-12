part of 'gestures.dart';

class CustomTouchGestureRecognizer extends ScaleGestureRecognizer {
  CustomTouchGestureRecognizer({
    Object? debugOwner,
    Set<PointerDeviceKind>? supportedDevices,
  }) : super(
          debugOwner: debugOwner,
          supportedDevices: supportedDevices,
        ) {
    _init();
  }

  // oneFingerPan
  GestureDragStartCallback? onOneFingerPanStart;
  GestureDragUpdateCallback? onOneFingerPanUpdate;
  GestureDragEndCallback? onOneFingerPanEnd;
  GestureDragCancelCallback? onOneFingerPanCancel;

  // twoFingerScale : scale + pan event
  GestureScaleStartCallback? onTwoFingerScaleStart;
  GestureScaleUpdateCallback? onTwoFingerScaleUpdate;
  GestureScaleEndCallback? onTwoFingerScaleEnd;

  // threeFingerVerticalDrag
  GestureDragStartCallback? onThreeFingerVerticalDragStart;
  GestureDragUpdateCallback? onThreeFingerVerticalDragUpdate;
  GestureDragEndCallback? onThreeFingerVerticalDragEnd;

  var _currentState = GestureState.none;
  Timer? _debounceTimer;

  void _init() {
    debugPrint("CustomTouchGestureRecognizer init");
    // onStart = (d) {};
    onUpdate = (d) {
      _debounceTimer?.cancel();
      if (d.pointerCount == 1 && _currentState != GestureState.oneFingerPan) {
        onOneFingerStartDebounce(d);
      } else if (d.pointerCount == 2 &&
          _currentState != GestureState.twoFingerScale) {
        onTwoFingerStartDebounce(d);
      } else if (d.pointerCount == 3 &&
          _currentState != GestureState.threeFingerVerticalDrag) {
        _currentState = GestureState.threeFingerVerticalDrag;
        if (onThreeFingerVerticalDragStart != null) {
          onThreeFingerVerticalDragStart!(
              DragStartDetails(globalPosition: d.localFocalPoint));
        }
        debugPrint("start threeFingerScale");
      }
      if (_currentState != GestureState.none) {
        switch (_currentState) {
          case GestureState.oneFingerPan:
            if (onOneFingerPanUpdate != null) {
              onOneFingerPanUpdate!(_getDragUpdateDetails(d));
            }
            break;
          case GestureState.twoFingerScale:
            if (onTwoFingerScaleUpdate != null) {
              onTwoFingerScaleUpdate!(d);
            }
            break;
          case GestureState.threeFingerVerticalDrag:
            if (onThreeFingerVerticalDragUpdate != null) {
              onThreeFingerVerticalDragUpdate!(_getDragUpdateDetails(d));
            }
            break;
          default:
            break;
        }
        return;
      }
    };
    onEnd = (d) {
      debugPrint("ScaleGestureRecognizer onEnd");
      _debounceTimer?.cancel();
      // end
      switch (_currentState) {
        case GestureState.oneFingerPan:
          debugPrint("OneFingerState.pan onEnd");
          if (onOneFingerPanEnd != null) {
            onOneFingerPanEnd!(_getDragEndDetails(d));
          }
          break;
        case GestureState.twoFingerScale:
          debugPrint("TwoFingerState.scale onEnd");
          if (onTwoFingerScaleEnd != null) {
            onTwoFingerScaleEnd!(d);
          }
          if (isSpecialHoldDragActive) {
            // If we are in special drag mode, we need to reset the state.
            // Otherwise, the next `onTwoFingerScaleUpdate()` will handle a wrong `focalPoint`.
            _currentState = GestureState.none;
            return;
          }
          break;
        case GestureState.threeFingerVerticalDrag:
          debugPrint("ThreeFingerState.vertical onEnd");
          if (onThreeFingerVerticalDragEnd != null) {
            onThreeFingerVerticalDragEnd!(_getDragEndDetails(d));
          }
          break;
        default:
          break;
      }
      _debounceTimer = Timer(Duration(milliseconds: 200), () {
        _currentState = GestureState.none;
      });
    };
  }

  // FIXME: This debounce logic is not working properly.
  // If we move our finger very fast, we won't be able to detect the "oneFingerPan" event sometimes.
  void onOneFingerStartDebounce(ScaleUpdateDetails d) {
    start(ScaleUpdateDetails d) {
      _currentState = GestureState.oneFingerPan;
      if (onOneFingerPanStart != null) {
        onOneFingerPanStart!(DragStartDetails(
            localPosition: d.localFocalPoint, globalPosition: d.focalPoint));
      }
    }

    if (_currentState != GestureState.none) {
      _debounceTimer = Timer(Duration(milliseconds: 200), () {
        start(d);
        debugPrint("debounce start oneFingerPan");
      });
    } else {
      start(d);
      debugPrint("start oneFingerPan");
    }
  }

  void onTwoFingerStartDebounce(ScaleUpdateDetails d) {
    start(ScaleUpdateDetails d) {
      _currentState = GestureState.twoFingerScale;
      if (onTwoFingerScaleStart != null) {
        onTwoFingerScaleStart!(ScaleStartDetails(
            localFocalPoint: d.localFocalPoint, focalPoint: d.focalPoint));
      }
    }

    if (_currentState == GestureState.threeFingerVerticalDrag) {
      _debounceTimer = Timer(Duration(milliseconds: 200), () {
        start(d);
        debugPrint("debounce start twoFingerScale");
      });
    } else {
      start(d);
      debugPrint("start twoFingerScale");
    }
  }

  DragUpdateDetails _getDragUpdateDetails(ScaleUpdateDetails d) =>
      DragUpdateDetails(
          globalPosition: d.focalPoint,
          localPosition: d.localFocalPoint,
          delta: d.focalPointDelta);

  DragEndDetails _getDragEndDetails(ScaleEndDetails d) =>
      DragEndDetails(velocity: d.velocity);

  @override
  void rejectGesture(int pointer) {
    super.rejectGesture(pointer);
    switch (_currentState) {
      case GestureState.oneFingerPan:
        if (onOneFingerPanCancel != null) {
          onOneFingerPanCancel!();
        }
        break;
      case GestureState.twoFingerScale:
        // Reset scale state if needed, currently self-contained
        break;
      case GestureState.threeFingerVerticalDrag:
        // Reset drag state if needed, currently self-contained
        break;
      default:
        break;
    }
    _currentState = GestureState.none;
  }
}
