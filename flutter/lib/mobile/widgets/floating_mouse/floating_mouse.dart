// This floating mouse widget simulates a physical mouse when connecting from mobile to desktop in touch mode.

import 'dart:async';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/utils/image.dart';
import 'package:provider/provider.dart';
part 'mouse_build.dart';
part 'mouse_move.dart';
part 'floating_mouse_state.dart';
part 'canvas_scroll.dart';
part 'mouse_body_scroll.dart';
part 'mouse_body.dart';
part 'painters.dart';

const int _kDotCount = 60;
const double _kDotAngle = 2 * pi / _kDotCount;
final Color _kDefaultColor = Colors.grey.withOpacity(0.7);
final Color _kDefaultHighlightColor = Colors.white24.withOpacity(0.7);
final Color _kTapDownColor = Colors.blue.withOpacity(0.7);
const double _baseMouseWidth = 112.0;
const double _baseMouseHeight = 138.0;
const double _kShowPressedScale = 1.2;
const double kScaleMax = 1.8;
const double kScaleMin = 0.8;

double? _tryParseCoordinateFromEvt(Map<String, dynamic>? evt, String key) {
  if (evt == null) return null;
  final coord = evt[key];
  if (coord == null) return null;
  return double.tryParse(coord);
}
