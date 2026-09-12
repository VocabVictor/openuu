import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'package:flutter/foundation.dart';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../model.dart';
import '../platform_model.dart';
import '../state_model.dart';
import '../input_modifier_utils.dart';
import '../relative_mouse_model.dart';
import '../../common.dart';
import '../../consts.dart';
part 'coords.dart';
part 'pointer_event.dart';
part 'key_events.dart';
part 'keyboard.dart';
part 'mouse_move.dart';
part 'key_mouse_send.dart';
part 'touch.dart';
part 'trackpad.dart';
part 'pointer_pos.dart';
part 'pointer.dart';
part 'input_model_core.dart';

/// Mouse button enum.
enum MouseButtons { left, right, wheel, back, forward }

const _kMouseEventDown = 'mousedown';
const _kMouseEventUp = 'mouseup';
const _kMouseEventMove = 'mousemove';
