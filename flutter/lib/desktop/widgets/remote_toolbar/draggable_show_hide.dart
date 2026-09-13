part of 'remote_toolbar.dart';

class _DraggableShowHide extends StatefulWidget {
  final String id;
  final FFI ffi;
  final SessionID sessionId;
  final RxDouble fraction;
  final Rx<_ToolbarEdge> edge;
  final Rxn<_ToolbarEdge> previewEdge;
  final Rxn<double> previewFraction;
  final Rxn<Size> toolbarSize;
  final VoidCallback markDragEpoch;
  final VoidCallback syncDockingOptionsAfterDragIfNeeded;
  final bool isHorizontal;
  // Whether multi-edge docking is enabled for this session (toggled in
  // Settings -> Other). When false, the drag handle slides the toolbar
  // horizontally on the top edge and never switches edges.
  final bool multiEdgeEnabled;
  final RxBool dragging;
  final ToolbarState toolbarState;
  final BorderRadius borderRadius;

  final Function(bool) setFullscreen;
  final Function() setMinimize;

  const _DraggableShowHide({
    Key? key,
    required this.id,
    required this.ffi,
    required this.sessionId,
    required this.fraction,
    required this.edge,
    required this.previewEdge,
    required this.previewFraction,
    required this.toolbarSize,
    required this.markDragEpoch,
    required this.syncDockingOptionsAfterDragIfNeeded,
    required this.isHorizontal,
    required this.multiEdgeEnabled,
    required this.dragging,
    required this.toolbarState,
    required this.setFullscreen,
    required this.setMinimize,
    required this.borderRadius,
  }) : super(key: key);

  @override
  State<_DraggableShowHide> createState() => _DraggableShowHideState();
}

class _DraggableShowHideState extends State<_DraggableShowHide> {
  double left = 0.0;
  double right = 1.0;
  Offset? _lastPointerDown;
  Offset? _dragGrabOffset;
  double? _dragLongAxisGrabOffset;
  Size? _dragToolbarSize;

  RxBool get collapse => widget.toolbarState.collapse;

  @override
  initState() {
    super.initState();

    final confLeft = double.tryParse(
        bind.mainGetLocalOption(key: kOptionRemoteMenubarDragLeft));
    if (confLeft == null) {
      bind.mainSetLocalOption(
          key: kOptionRemoteMenubarDragLeft, value: left.toString());
    } else {
      left = confLeft;
    }
    final confRight = double.tryParse(
        bind.mainGetLocalOption(key: kOptionRemoteMenubarDragRight));
    if (confRight == null) {
      bind.mainSetLocalOption(
          key: kOptionRemoteMenubarDragRight, value: right.toString());
    } else {
      right = confRight;
    }
  }

  Widget _buildDraggable(BuildContext context) {
    return Listener(
      onPointerDown: (event) => _lastPointerDown = event.position,
      child: Draggable(
        // When multi-edge docking is off the toolbar stays on the top edge,
        // so lock the feedback to horizontal motion — otherwise the handle
        // floats away from the top while dragging and the toolbar looks
        // unmoored. When multi-edge is on we need 2D drag for snap-to-edge.
        axis: widget.multiEdgeEnabled ? null : Axis.horizontal,
        child: Icon(
          widget.isHorizontal ? Icons.drag_indicator : Icons.drag_handle,
          size: UiSession.toolbarHandleIconSize,
          color: UiColor.of(context).faint,
        ),
        feedback: widget,
        onDragStarted: () {
          widget.markDragEpoch();
          final pointerDown = _lastPointerDown;
          if (pointerDown != null) {
            _ensureDragGrabOffset(pointerDown);
          }
          widget.dragging.value = true;
          // Seed the preview at the current docked edge/fraction so something
          // shows the instant the drag begins, before the first onDragUpdate.
          widget.previewEdge.value = widget.edge.value;
          widget.previewFraction.value = widget.fraction.value;
        },
        onDragUpdate: (details) {
          _updatePreview(details.globalPosition);
        },
        onDragEnd: (_) => _commitPreview(),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final ButtonStyle buttonStyle = ButtonStyle(
      minimumSize: MaterialStateProperty.all(const Size(0, 0)),
      padding: MaterialStateProperty.all(EdgeInsets.zero),
      foregroundColor: WidgetStatePropertyAll(
          _ToolbarTheme.iconColor(context, _ToolbarTheme.blueColor)),
    );
    final isFullscreen = stateGlobal.fullscreen;
    const double iconSize = UiSession.toolbarHandleIconSize;

    buttonWrapper(VoidCallback? onPressed, Widget child,
        {Color? hoverColor}) {
      hoverColor ??= _ToolbarTheme.hoverBlueColor(context);
      final bgColor = buttonStyle.backgroundColor?.resolve({});
      return TextButton(
        onPressed: onPressed,
        child: child,
        style: buttonStyle.copyWith(
          backgroundColor: MaterialStateProperty.resolveWith((states) {
            if (states.contains(MaterialState.hovered)) {
              return bgColor ?? hoverColor;
            }
            return bgColor;
          }),
        ),
      );
    }

    final axis = widget.isHorizontal ? Axis.horizontal : Axis.vertical;
    final child = Flex(
      direction: axis,
      mainAxisSize: MainAxisSize.min,
      children: [
        _buildDraggable(context),
        Obx(() => collapse.isTrue
            ? _MinimizedMonitorSwitchButton(id: widget.id, ffi: widget.ffi)
            : const Offstage()),
        Obx(() => buttonWrapper(
              () {
                widget.setFullscreen(!isFullscreen.value);
              },
              Tooltip(
                message: translate(
                    isFullscreen.isTrue ? 'Exit Fullscreen' : 'Fullscreen'),
                child: Icon(
                  isFullscreen.isTrue
                      ? Icons.fullscreen_exit
                      : Icons.fullscreen,
                  size: iconSize,
                ),
              ),
            )),
        if (!isMacOS)
          Obx(() => Offstage(
                offstage: isFullscreen.isFalse,
                child: buttonWrapper(
                  widget.setMinimize,
                  Tooltip(
                    message: translate('Minimize'),
                    child: Icon(
                      Icons.remove,
                      size: iconSize,
                    ),
                  ),
                ),
              )),
        buttonWrapper(
          () => setState(() {
            widget.toolbarState.switchCollapse(widget.sessionId);
          }),
          Obx((() => Tooltip(
                message: translate(
                    collapse.isFalse ? 'Hide Toolbar' : 'Show Toolbar'),
                child: Icon(
                  _toolbarCollapseIcon(widget.edge.value, collapse.isTrue),
                  size: iconSize,
                ),
              ))),
        ),],
    );
    return TextButtonTheme(
      data: TextButtonThemeData(style: buttonStyle),
      child: Container(
        decoration: BoxDecoration(
          color: _ToolbarTheme.barColor(context),
          border: Border.all(
            color: _ToolbarTheme.borderColor(context),
            width: 1,
          ),
          borderRadius: widget.borderRadius,
        ),
        child: SizedBox(
          height: widget.isHorizontal ? UiSession.toolbarHandleThickness : null,
          width: widget.isHorizontal ? null : UiSession.toolbarHandleThickness,
          child: child,
        ),
      ),
    );
  }
}
