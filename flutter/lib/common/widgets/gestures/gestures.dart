import 'dart:async';
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common/widgets/remote_input.dart';
part 'custom_touch.dart';
part 'hold_tap_move.dart';
part 'double_finer_tap.dart';
part 'mixin_detector.dart';

enum GestureState {
  none,
  oneFingerPan,
  twoFingerScale,
  threeFingerVerticalDrag
}
