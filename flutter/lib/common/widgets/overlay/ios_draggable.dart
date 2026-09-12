part of 'overlay.dart';

class IOSDraggable extends StatefulWidget {
  const IOSDraggable(
      {Key? key,
      this.chatModel,
      required this.position,
      required this.width,
      required this.height,
      required this.builder})
      : super(key: key);

  final DraggableKeyPosition position;
  final ChatModel? chatModel;
  final double width;
  final double height;
  final Widget Function(BuildContext) builder;

  @override
  IOSDraggableState createState() =>
      IOSDraggableState(chatModel, width, height);
}

class IOSDraggableState extends State<IOSDraggable> {
  late ChatModel? _chatModel;
  late double _width;
  late double _height;
  bool _keyboardVisible = false;
  double _saveHeight = 0;
  double _lastBottomHeight = 0;

  IOSDraggableState(ChatModel? chatModel, double w, double h) {
    _chatModel = chatModel;
    _width = w;
    _height = h;
  }

  DraggableKeyPosition get position => widget.position;

  checkKeyboard() {
    final bottomHeight = MediaQuery.of(context).viewInsets.bottom;
    final currentVisible = bottomHeight != 0;

    // save
    if (!_keyboardVisible && currentVisible) {
      _saveHeight = position.pos.dy;
    }

    // reset
    if (_lastBottomHeight > 0 && bottomHeight == 0) {
      setState(() {
        position.update(Offset(position.pos.dx, _saveHeight));
      });
    }

    // onKeyboardVisible
    if (_keyboardVisible && currentVisible) {
      final sumHeight = bottomHeight + _height;
      final contextHeight = MediaQuery.of(context).size.height;
      if (sumHeight + position.pos.dy > contextHeight) {
        final y = contextHeight - sumHeight;
        setState(() {
          position.update(Offset(position.pos.dx, y));
        });
      }
    }

    _keyboardVisible = currentVisible;
    _lastBottomHeight = bottomHeight;
  }

  @override
  void initState() {
    super.initState();
    position.tryAdjust(_width, _height, 1);
  }

  @override
  Widget build(BuildContext context) {
    checkKeyboard();
    return Stack(
      children: [
        Positioned(
          left: position.pos.dx,
          top: position.pos.dy,
          child: GestureDetector(
            onPanUpdate: (details) {
              setState(() {
                position.update(position.pos + details.delta);
              });
              _chatModel?.setChatWindowPosition(position.pos);
            },
            child: Material(
              child: Container(
                width: _width,
                height: _height,
                decoration:
                    BoxDecoration(border: Border.all(color: MyTheme.border)),
                child: widget.builder(context),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
