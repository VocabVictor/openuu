// These floating mouse widgets are used to simulate a physical mouse
// when "mobile" -> "desktop" in mouse mode.
// This file does not contain whole mouse widgets, it only contains
// parts that help to control, such as wheel scroll and wheel button.

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/remote_input.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
part 'wheel.dart';
part 'joystick.dart';
part 'left_right_button_position.dart';
part 'left_right_button.dart';

// Used for the wheel button and wheel scroll widgets
const double _kSpaceToHorizontalEdge = 25;
const double _wheelWidth = 50;
const double _wheelHeight = 162;
// Used for the left/right button widgets
const double _kSpaceToVerticalEdge = 15;
const double _kSpaceBetweenLeftRightButtons = 40;
const double _kLeftRightButtonWidth = 55;
const double _kLeftRightButtonHeight = 40;
const double _kBorderWidth = 1;
final Color _kDefaultBorderColor = Colors.white.withOpacity(0.7);
final Color _kDefaultColor = Colors.black.withOpacity(0.4);
final Color _kTapDownColor = Colors.blue.withOpacity(0.7);
final Color _kWidgetHighlightColor = Colors.white.withOpacity(0.9);
const int _kInputTimerIntervalMillis = 100;

class FloatingMouseWidgets extends StatefulWidget {
  final FFI ffi;
  const FloatingMouseWidgets({
    super.key,
    required this.ffi,
  });

  @override
  State<FloatingMouseWidgets> createState() => _FloatingMouseWidgetsState();
}

class _FloatingMouseWidgetsState extends State<FloatingMouseWidgets> {
  InputModel get _inputModel => widget.ffi.inputModel;
  CursorModel get _cursorModel => widget.ffi.cursorModel;
  late final VirtualMouseMode _virtualMouseMode;

  @override
  void initState() {
    super.initState();
    _virtualMouseMode = widget.ffi.ffiModel.virtualMouseMode;
    _virtualMouseMode.addListener(_onVirtualMouseModeChanged);
    _cursorModel.blockEvents = false;
    isSpecialHoldDragActive = false;
  }

  void _onVirtualMouseModeChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void dispose() {
    _virtualMouseMode.removeListener(_onVirtualMouseModeChanged);
    super.dispose();
    _cursorModel.blockEvents = false;
    isSpecialHoldDragActive = false;
  }

  @override
  Widget build(BuildContext context) {
    final virtualMouseMode = _virtualMouseMode;
    if (!virtualMouseMode.showVirtualMouse) {
      return const Offstage();
    }
    return Stack(
      children: [
        FloatingWheel(
          inputModel: _inputModel,
          cursorModel: _cursorModel,
        ),
        if (virtualMouseMode.showVirtualJoystick)
          VirtualJoystick(
            cursorModel: _cursorModel,
            inputModel: _inputModel,
          ),
        FloatingLeftRightButton(
          isLeft: true,
          inputModel: _inputModel,
          cursorModel: _cursorModel,
        ),
        FloatingLeftRightButton(
          isLeft: false,
          inputModel: _inputModel,
          cursorModel: _cursorModel,
        ),
      ],
    );
  }
}
