part of 'overlay.dart';

class DraggableKeyPosition {
  final String key;
  Offset _pos;
  late Debouncer<int> _debouncerStore;
  DraggableKeyPosition(this.key)
      : _pos = DraggablePositions.kInvalidDraggablePosition;

  get pos => _pos;

  _loadPosition(String k) {
    final value = bind.getLocalFlutterOption(k: k);
    if (value.isNotEmpty) {
      final parts = value.split(',');
      if (parts.length == 2) {
        return Offset(double.parse(parts[0]), double.parse(parts[1]));
      }
    }
    return DraggablePositions.kInvalidDraggablePosition;
  }

  load() {
    _pos = _loadPosition(key);
    _debouncerStore = Debouncer<int>(const Duration(milliseconds: 500),
        onChanged: (v) => _store(), initialValue: 0);
  }

  update(Offset pos) {
    _pos = pos;
    _triggerStore();
  }

  // Adjust position to keep it in the screen
  // Only used for desktop and web desktop
  tryAdjust(double w, double h, double scale) {
    final size = MediaQuery.of(Get.context!).size;
    w = w * scale;
    h = h * scale;
    double x = _pos.dx;
    double y = _pos.dy;
    if (x + w > size.width) {
      x = size.width - w;
    }
    final tabBarHeight = isDesktop ? kDesktopRemoteTabBarHeight : 0;
    if (y + h > (size.height - tabBarHeight)) {
      y = size.height - tabBarHeight - h;
    }
    if (x < 0) {
      x = 0;
    }
    if (y < 0) {
      y = 0;
    }
    if (x != _pos.dx || y != _pos.dy) {
      update(Offset(x, y));
    }
  }

  isInvalid() {
    return _pos == DraggablePositions.kInvalidDraggablePosition;
  }

  _triggerStore() => _debouncerStore.value = _debouncerStore.value + 1;
  _store() {
    bind.setLocalFlutterOption(k: key, v: '${_pos.dx},${_pos.dy}');
  }
}

class DraggablePositions {
  static const kChatWindow = 'draggablePositionChat';
  static const kMobileActions = 'draggablePositionMobile';
  static const kIOSDraggable = 'draggablePositionIOS';

  static const kInvalidDraggablePosition = Offset(-999999, -999999);
  final chatWindow = DraggableKeyPosition(kChatWindow);
  final mobileActions = DraggableKeyPosition(kMobileActions);
  final iOSDraggable = DraggableKeyPosition(kIOSDraggable);

  load() {
    chatWindow.load();
    mobileActions.load();
    iOSDraggable.load();
  }
}

DraggablePositions draggablePositions = DraggablePositions();

class Draggable extends StatefulWidget {
  Draggable(
      {Key? key,
      this.checkKeyboard = false,
      this.checkScreenSize = false,
      required this.position,
      required this.width,
      required this.height,
      this.chatModel,
      required this.builder})
      : super(key: key);

  final bool checkKeyboard;
  final bool checkScreenSize;
  final DraggableKeyPosition position;
  final double width;
  final double height;
  final ChatModel? chatModel;
  final Widget Function(BuildContext, GestureDragUpdateCallback) builder;

  @override
  State<StatefulWidget> createState() => _DraggableState(chatModel);
}

class _DraggableState extends State<Draggable> {
  late ChatModel? _chatModel;
  bool _keyboardVisible = false;
  double _saveHeight = 0;
  double _lastBottomHeight = 0;

  _DraggableState(ChatModel? chatModel) {
    _chatModel = chatModel;
  }

  get position => widget.position.pos;

  void onPanUpdate(DragUpdateDetails d) {
    final offset = d.delta;
    final size = MediaQuery.of(context).size;
    double x = 0;
    double y = 0;

    if (position.dx + offset.dx + widget.width > size.width) {
      x = size.width - widget.width;
    } else if (position.dx + offset.dx < 0) {
      x = 0;
    } else {
      x = position.dx + offset.dx;
    }

    if (position.dy + offset.dy + widget.height > size.height) {
      y = size.height - widget.height;
    } else if (position.dy + offset.dy < 0) {
      y = 0;
    } else {
      y = position.dy + offset.dy;
    }
    setState(() {
      widget.position.update(Offset(x, y));
    });
    _chatModel?.setChatWindowPosition(position);
  }

  checkScreenSize() {
    // Ensure the draggable always stays within current screen bounds
    widget.position.tryAdjust(widget.width, widget.height, 1);
  }

  checkKeyboard() {
    final bottomHeight = MediaQuery.of(context).viewInsets.bottom;
    final currentVisible = bottomHeight != 0;

    // save
    if (!_keyboardVisible && currentVisible) {
      _saveHeight = position.dy;
    }

    // reset
    if (_lastBottomHeight > 0 && bottomHeight == 0) {
      setState(() {
        widget.position.update(Offset(position.dx, _saveHeight));
      });
    }

    // onKeyboardVisible
    if (_keyboardVisible && currentVisible) {
      final sumHeight = bottomHeight + widget.height;
      final contextHeight = MediaQuery.of(context).size.height;
      if (sumHeight + position.dy > contextHeight) {
        final y = contextHeight - sumHeight;
        setState(() {
          widget.position.update(Offset(position.dx, y));
        });
      }
    }

    _keyboardVisible = currentVisible;
    _lastBottomHeight = bottomHeight;
  }

  @override
  Widget build(BuildContext context) {
    if (widget.checkKeyboard) {
      checkKeyboard();
    }
    if (widget.checkScreenSize) {
      checkScreenSize();
    }
    return Stack(children: [
      Positioned(
          top: position.dy,
          left: position.dx,
          width: widget.width,
          height: widget.height,
          child: widget.builder(context, onPanUpdate))
    ]);
  }
}
