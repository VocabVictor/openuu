part of 'remote_toolbar.dart';

extension _RemoteToolbarLayout on _RemoteToolbarState {
  Widget _buildDraggableCollapse(
      BuildContext context, _ToolbarEdge edge, bool isHorizontal) {
    return Obx(() {
      if (collapse.isFalse && _dragging.isFalse) {
        triggerAutoHide();
      }
      final borderRadius = _collapseHandleBorderRadius(edge);
      return Offstage(
        offstage: _dragging.isTrue,
        child: Material(
          elevation: _ToolbarTheme.elevation,
          shadowColor: MyTheme.color(context).shadow,
          borderRadius: borderRadius,
          child: _DraggableShowHide(
            id: widget.id,
            ffi: widget.ffi,
            sessionId: widget.ffi.sessionId,
            dragging: _dragging,
            fraction: _fraction,
            edge: _edge,
            previewEdge: _previewEdge,
            previewFraction: _previewFraction,
            toolbarSize: _toolbarSize,
            markDragEpoch: _markToolbarDragEpoch,
            syncDockingOptionsAfterDragIfNeeded:
                _syncDockingOptionsAfterDragIfNeeded,
            isHorizontal: isHorizontal,
            multiEdgeEnabled: _multiEdgeEnabled.value,
            toolbarState: widget.state,
            setFullscreen: _setFullscreen,
            setMinimize: _minimize,
            borderRadius: borderRadius,
          ),
        ),
      );
    });
  }

  Widget _buildToolbar(
      BuildContext context, _ToolbarEdge edge, bool isHorizontal) {
    final List<Widget> toolbarItems = [];
    toolbarItems.add(_PinMenu(state: widget.state));
    toolbarItems.add(Obx(() {
      final privacyModeState = PrivacyModeState.find(widget.id);
      if ((privacyModeState.isEmpty ||
              allowDisplaySwitchInPrivacyMode(pi, privacyModeState.value)) &&
          pi.displaysCount.value > 1 &&
          mainGetLocalBoolOptionSync(kOptionAllowMonitorSwitchMainToolbar)) {
        return _MainMonitorSwitchButton(id: widget.id, ffi: widget.ffi);
      } else {
        return const Offstage();
      }
    }));
    toolbarItems.add(_MobileActionMenu(ffi: widget.ffi));

    toolbarItems.add(Obx(() {
      final privacyModeState = PrivacyModeState.find(widget.id);
      if ((privacyModeState.isEmpty ||
              allowDisplaySwitchInPrivacyMode(pi, privacyModeState.value)) &&
          pi.displaysCount.value > 1) {
        return _MonitorMenu(
            id: widget.id,
            ffi: widget.ffi,
            edge: edge,
            setRemoteState: widget.setRemoteState);
      } else {
        return Offstage();
      }
    }));

    toolbarItems
        .add(_ControlMenu(id: widget.id, ffi: widget.ffi, state: widget.state));
    toolbarItems.add(_DisplayMenu(
      id: widget.id,
      ffi: widget.ffi,
      state: widget.state,
      setFullscreen: _setFullscreen,
    ));
    // Do not show keyboard for camera connection type.
    if (widget.ffi.connType == ConnType.defaultConn) {
      toolbarItems.add(_KeyboardMenu(id: widget.id, ffi: widget.ffi));
    }
    toolbarItems.add(_ChatMenu(id: widget.id, ffi: widget.ffi));
    toolbarItems.add(_VoiceCallMenu(id: widget.id, ffi: widget.ffi));
    toolbarItems.add(_RecordMenu());
    toolbarItems.add(_CloseMenu(id: widget.id, ffi: widget.ffi));
    final toolbarBorderRadius =
        BorderRadius.all(Radius.circular(_ToolbarTheme.barRadius));
    // innerAxis: how the toolbar icons themselves flow.
    // outerAxis: how the toolbar block and the handle stack against each other
    // (perpendicular to the dock edge, so the handle hangs off the interior face).
    final innerAxis = isHorizontal ? Axis.horizontal : Axis.vertical;
    final outerAxis = isHorizontal ? Axis.vertical : Axis.horizontal;
    final spacer = isHorizontal
        ? SizedBox(width: UiSession.toolbarPaddingX - _ToolbarTheme.buttonHMargin)
        : SizedBox(height: UiSession.toolbarPaddingX - _ToolbarTheme.buttonHMargin);
    final toolbarMaterial = Container(
      decoration: _ToolbarTheme.barDecoration(context),
      child: Material(
        type: MaterialType.transparency,
        borderRadius: toolbarBorderRadius,
        child: SingleChildScrollView(
          scrollDirection: innerAxis,
          child: Theme(
            data: themeData(),
            child: Flex(
              direction: innerAxis,
              mainAxisSize: MainAxisSize.min,
              children: [
                spacer,
                ...toolbarItems,
                spacer,
              ],
            ),
          ),
        ),
      ),
    );
    final handle = _buildDraggableCollapse(context, edge, isHorizontal);
    // The handle hangs off the interior face of the toolbar (away from the
    // docked edge), centered along that face by the Flex's default cross-axis
    // alignment, so the icons themselves sit flush against the docked edge.
    final children = (edge == _ToolbarEdge.top || edge == _ToolbarEdge.left)
        ? [toolbarMaterial, handle]
        : [handle, toolbarMaterial];
    return Flex(
      direction: outerAxis,
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
  }

  ThemeData themeData() {
    return Theme.of(context).copyWith(
      menuButtonTheme: MenuButtonThemeData(
        style: ButtonStyle(
          minimumSize: const WidgetStatePropertyAll(
              Size(UiSession.toolbarMenuMinWidth, UiSpace.menuItemHeight)),
          padding: const WidgetStatePropertyAll(
              EdgeInsets.symmetric(horizontal: UiSpace.menuItemPaddingX)),
          textStyle: WidgetStatePropertyAll(UiType.sidebarItem),
          foregroundColor:
              WidgetStatePropertyAll(UiColor.of(context).text),
          overlayColor:
              WidgetStatePropertyAll(UiColor.of(context).settingsRowHover),
          shape: MaterialStatePropertyAll(RoundedRectangleBorder(
              borderRadius:
                  BorderRadius.circular(_ToolbarTheme.menuButtonBorderRadius))),
        ),
      ),
      dividerTheme: DividerThemeData(
        space: _ToolbarTheme.dividerSpaceToAction,
        color: _ToolbarTheme.dividerColor(context),
      ),
      menuBarTheme: MenuBarThemeData(
          style: MenuStyle(
        padding: MaterialStatePropertyAll(EdgeInsets.zero),
        elevation: MaterialStatePropertyAll(0),
        shape: MaterialStatePropertyAll(BeveledRectangleBorder()),
      ).copyWith(
              backgroundColor:
                  Theme.of(context).menuBarTheme.style?.backgroundColor)),
    );
  }
}
