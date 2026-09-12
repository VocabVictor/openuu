part of 'gesture_help.dart';

class _GestureHelpState extends State<GestureHelp> {
  late int _selectedIndex;
  late bool _touchMode;
  final VirtualMouseMode _virtualMouseMode;

  _GestureHelpState(bool touchMode, VirtualMouseMode virtualMouseMode)
      : _virtualMouseMode = virtualMouseMode {
    _touchMode = touchMode;
    _selectedIndex = _touchMode ? 1 : 0;
  }

  /// Helper to exit relative mouse mode when certain conditions are met.
  /// This reduces code duplication across multiple UI callbacks.
  void _exitRelativeMouseModeIf(bool condition) {
    if (condition) {
      widget.inputModel?.setRelativeMouseMode(false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final size = MediaQuery.of(context).size;
    final space = 12.0;
    var width = size.width - 2 * space;
    final minWidth = 90;
    if (size.width > minWidth + 2 * space) {
      final n = (size.width / (minWidth + 2 * space)).floor();
      width = size.width / n - 2 * space;
    }
    return Center(
        child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 12.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Center(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      ToggleSwitch(
                        initialLabelIndex: _selectedIndex,
                        activeFgColor: Colors.white,
                        inactiveFgColor: Colors.white60,
                        activeBgColor: [MyTheme.accent],
                        inactiveBgColor: Theme.of(context).hintColor,
                        totalSwitches: 2,
                        minWidth: 150,
                        fontSize: 15,
                        iconSize: 18,
                        labels: [
                          translate("Mouse mode"),
                          translate("Touch mode")
                        ],
                        icons: [Icons.mouse, Icons.touch_app],
                        onToggle: (index) {
                          setState(() {
                            if (_selectedIndex != index) {
                              _selectedIndex = index ?? 0;
                              _touchMode = index == 0 ? false : true;
                              widget.onTouchModeChange(_touchMode);
                              // Exit relative mouse mode when switching to touch mode
                              _exitRelativeMouseModeIf(_touchMode);
                            }
                          });
                        },
                      ),
                      Transform.translate(
                        offset: const Offset(-10.0, 0.0),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Checkbox(
                              value: _virtualMouseMode.showVirtualMouse,
                              onChanged: (value) async {
                                if (value == null) return;
                                await _virtualMouseMode.toggleVirtualMouse();
                                // Exit relative mouse mode when virtual mouse is hidden
                                _exitRelativeMouseModeIf(
                                    !_virtualMouseMode.showVirtualMouse);
                                setState(() {});
                              },
                            ),
                            InkWell(
                              onTap: () async {
                                await _virtualMouseMode.toggleVirtualMouse();
                                // Exit relative mouse mode when virtual mouse is hidden
                                _exitRelativeMouseModeIf(
                                    !_virtualMouseMode.showVirtualMouse);
                                setState(() {});
                              },
                              child: Text(translate('Show virtual mouse')),
                            ),
                          ],
                        ),
                      ),
                      if (_touchMode && _virtualMouseMode.showVirtualMouse)
                        Padding(
                          // Indent "Virtual mouse size"
                          padding: const EdgeInsets.only(left: 24.0),
                          child: SizedBox(
                            width: 260,
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Padding(
                                  padding: const EdgeInsets.only(
                                      top: 0.0, bottom: 0),
                                  child: Text(translate('Virtual mouse size')),
                                ),
                                Transform.translate(
                                  offset: Offset(-0.0, -6.0),
                                  child: Row(
                                    children: [
                                      Padding(
                                        padding:
                                            const EdgeInsets.only(left: 0.0),
                                        child: Text(translate('Small')),
                                      ),
                                      Expanded(
                                        child: Slider(
                                          value: _virtualMouseMode
                                              .virtualMouseScale,
                                          min: 0.8,
                                          max: 1.8,
                                          divisions: 10,
                                          onChanged: (value) {
                                            _virtualMouseMode
                                                .setVirtualMouseScale(value);
                                            setState(() {});
                                          },
                                        ),
                                      ),
                                      Padding(
                                        padding:
                                            const EdgeInsets.only(right: 16.0),
                                        child: Text(translate('Large')),
                                      ),
                                    ],
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      if (!_touchMode && _virtualMouseMode.showVirtualMouse)
                        Transform.translate(
                          offset: const Offset(-10.0, -12.0),
                          child: Padding(
                              // Indent "Show virtual joystick"
                              padding: const EdgeInsets.only(left: 24.0),
                              child: Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Checkbox(
                                    value:
                                        _virtualMouseMode.showVirtualJoystick,
                                    onChanged: (value) async {
                                      if (value == null) return;
                                      await _virtualMouseMode
                                          .toggleVirtualJoystick();
                                      // Exit relative mouse mode when joystick is hidden
                                      _exitRelativeMouseModeIf(
                                          !_virtualMouseMode
                                              .showVirtualJoystick);
                                      setState(() {});
                                    },
                                  ),
                                  InkWell(
                                    onTap: () async {
                                      await _virtualMouseMode
                                          .toggleVirtualJoystick();
                                      // Exit relative mouse mode when joystick is hidden
                                      _exitRelativeMouseModeIf(
                                          !_virtualMouseMode
                                              .showVirtualJoystick);
                                      setState(() {});
                                    },
                                    child: Text(
                                        translate("Show virtual joystick")),
                                  ),
                                ],
                              )),
                        ),
                      // Relative mouse mode option - only visible when joystick is shown
                      if (!_touchMode &&
                          _virtualMouseMode.showVirtualMouse &&
                          _virtualMouseMode.showVirtualJoystick &&
                          widget.inputModel != null)
                        Obx(() => Transform.translate(
                              offset: const Offset(-10.0, -24.0),
                              child: Padding(
                                  // Indent further for 'Relative mouse mode'
                                  padding: const EdgeInsets.only(left: 48.0),
                                  child: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Checkbox(
                                        value: widget.inputModel!
                                            .relativeMouseMode.value,
                                        onChanged: (value) {
                                          if (value == null) return;
                                          widget.inputModel!
                                              .setRelativeMouseMode(value);
                                        },
                                      ),
                                      InkWell(
                                        onTap: () {
                                          widget.inputModel!
                                              .toggleRelativeMouseMode();
                                        },
                                        child: Text(
                                            translate('Relative mouse mode')),
                                      ),
                                    ],
                                  )),
                            )),
                    ],
                  ),
                ),
                Container(
                    child: Wrap(
                  spacing: space,
                  runSpacing: 2 * space,
                  children: _touchMode
                      ? [
                          GestureInfo(
                              width,
                              GestureIcons.iconMobileTouch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePressHold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFSwipeRight,
                              translate("One-Finger Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFThreeFingers,
                              translate("Three-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFDrag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePinch,
                              translate("Pinch to Zoom"),
                              translate("Canvas Zoom")),
                        ]
                      : [
                          GestureInfo(
                              width,
                              GestureIcons.iconMobileTouch,
                              translate("One-Finger Tap"),
                              translate("Left Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePressHold,
                              translate("One-Long Tap"),
                              translate("Right Mouse")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFSwipeRight,
                              translate("Double Tap & Move"),
                              translate("Mouse Drag")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFThreeFingers,
                              translate("Three-Finger vertically"),
                              translate("Mouse Wheel")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGestureFDrag,
                              translate("Two-Finger Move"),
                              translate("Canvas Move")),
                          GestureInfo(
                              width,
                              GestureIcons.iconGesturePinch,
                              translate("Pinch to Zoom"),
                              translate("Canvas Zoom")),
                        ],
                )),
              ],
            )));
  }
}
