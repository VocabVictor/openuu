import 'package:auto_size_text/auto_size_text.dart';
import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';
import 'package:provider/provider.dart';

import '../../../consts.dart';
import '../../../desktop/widgets/tabbar_widget.dart';
import '../../../models/chat_model.dart';
import '../../../models/model.dart';
import '../chat_page.dart';
part 'chat_window.dart';
part 'draggable.dart';

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

class QualityMonitor extends StatelessWidget {
  final QualityMonitorModel qualityMonitorModel;
  QualityMonitor(this.qualityMonitorModel);

  Widget _row(String info, String? value, {Color? rightColor}) {
    return Row(
      children: [
        Expanded(
            flex: 8,
            child: AutoSizeText(info,
                style: TextStyle(color: Color.fromARGB(255, 210, 210, 210)),
                textAlign: TextAlign.right,
                maxLines: 1)),
        Spacer(flex: 1),
        Expanded(
            flex: 8,
            child: AutoSizeText(value ?? '',
                style: TextStyle(color: rightColor ?? Colors.white),
                maxLines: 1)),
      ],
    );
  }

  @override
  Widget build(BuildContext context) => ChangeNotifierProvider.value(
      value: qualityMonitorModel,
      child: Consumer<QualityMonitorModel>(
          builder: (context, qualityMonitorModel, child) => qualityMonitorModel
                  .show
              ? Container(
                  constraints: BoxConstraints(maxWidth: 200),
                  padding: const EdgeInsets.all(8),
                  color: MyTheme.canvasColor.withAlpha(150),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _row("Speed", qualityMonitorModel.data.speed ?? '-'),
                      _row("FPS", qualityMonitorModel.data.fps ?? '-'),
                      // let delay be 0 if fps is 0
                      _row(
                          "Delay",
                          "${qualityMonitorModel.data.delay == null ? '-' : (qualityMonitorModel.data.fps ?? "").replaceAll(' ', '').replaceAll('0', '').isEmpty ? 0 : qualityMonitorModel.data.delay}ms",
                          rightColor: Colors.green),
                      _row("Target Bitrate",
                          "${qualityMonitorModel.data.targetBitrate ?? '-'}kb"),
                      _row(
                          "Codec", qualityMonitorModel.data.codecFormat ?? '-'),
                      _row("Chroma", qualityMonitorModel.data.chroma ?? '-'),
                      if (qualityMonitorModel.webrtcTransport != null)
                        _row("Transport",
                            qualityMonitorModel.webrtcTransport!),
                    ],
                  ),
                )
              : const SizedBox.shrink()));
}

class BlockableOverlayState extends OverlayKeyState {
  final _middleBlocked = false.obs;

  VoidCallback? onMiddleBlockedClick; // to-do use listener

  RxBool get middleBlocked => _middleBlocked;

  void addMiddleBlockedListener(void Function(bool) cb) {
    _middleBlocked.listen(cb);
  }

  void setMiddleBlocked(bool blocked) {
    if (blocked != _middleBlocked.value) {
      _middleBlocked.value = blocked;
    }
  }

  void applyFfi(FFI ffi) {
    ffi.dialogManager.setOverlayState(this);
    ffi.chatModel.setOverlayState(this);
    // make remote page penetrable automatically, effective for chat over remote
    onMiddleBlockedClick = () {
      setMiddleBlocked(false);
    };
  }
}

class BlockableOverlay extends StatelessWidget {
  final Widget underlying;
  final List<OverlayEntry>? upperLayer;

  final BlockableOverlayState state;

  BlockableOverlay(
      {required this.underlying, required this.state, this.upperLayer});

  @override
  Widget build(BuildContext context) {
    final initialEntries = [
      OverlayEntry(builder: (_) => underlying),

      /// middle layer
      OverlayEntry(
          builder: (context) => Obx(() => Listener(
              onPointerDown: (_) {
                state.onMiddleBlockedClick?.call();
              },
              child: Container(
                  color:
                      state.middleBlocked.value ? Colors.transparent : null)))),
    ];

    if (upperLayer != null) {
      initialEntries.addAll(upperLayer!);
    }

    /// set key
    return Overlay(key: state.key, initialEntries: initialEntries);
  }
}
