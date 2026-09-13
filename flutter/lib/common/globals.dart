import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:uuid/uuid.dart';

import '../models/platform_model.dart';

import 'package:flutter_hbb/native/common.dart';
import 'package:flutter_hbb/utils/http_service.dart' as http;
import 'windows_misc.dart';

final globalKey = GlobalKey<NavigatorState>();

final navigationBarKey = GlobalKey();

final isAndroid = isAndroid_;

final isIOS = isIOS_;

final isWindows = isWindows_;

final isMacOS = isMacOS_;

final isLinux = isLinux_;

final isDesktop = isDesktop_;

final isWeb = isWeb_;

final isWebDesktop = isWebDesktop_;

final isWebOnWindows = isWebOnWindows_;

final isWebOnLinux = isWebOnLinux_;

final isWebOnMacOs = isWebOnMacOS_;

var isMobile = isAndroid || isIOS;

var version = '';

int androidVersion = 0;

// Tolerance used for floating-point position comparisons to avoid precision errors.
const double _kPositionEpsilon = 1e-6;

bool get isMainDesktopWindow =>
    desktopType == DesktopType.main || desktopType == DesktopType.cm;

/// Check if the app is running with single view mode.
bool isSingleViewApp() {
  return desktopType == DesktopType.cm;
}

/// * debug or test only, DO NOT enable in release build
bool isTest = false;

typedef F = String Function(String);

typedef FMethod = String Function(String, dynamic);

typedef StreamEventHandler = Future<void> Function(Map<String, dynamic>);

typedef SessionID = UuidValue;

final iconHardDrive = MemoryImage(Uint8List.fromList(base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAMgAAADICAMAAACahl6sAAAAmVBMVEUAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAjHWqVAAAAMnRSTlMAv0BmzLJNXlhiUu2fxXDgu7WuSUUe29LJvpqUjX53VTstD7ilNujCqTEk5IYH+vEoFjKvAagAAAPpSURBVHja7d0JbhpBEIXhB3jYzb5vBgzYgO04df/DJXGUKMwU9ECmZ6pQfSfw028LCXW3YYwxxhhjjDHGGGOM0eZ9VV1MckdKWLM1bRQ/35GW/WxHHu1me6ShuyHvNl34VhlTKsYVeDWj1EzgUZ1S1DrAk/UDparZgxd9Sl0BHnxSBhpI3jfKQG2FpLUpE69I2ILikv1nsvygjBwPSNKYMlNHggqUoSKS80AZCnwHqQ1zCRvW+CRegwRFeFAMKKrtM8gTPJlzSfwFgT9dJom3IDN4VGaSeAryAK8m0SSeghTg1ZYiql6CjBDhO8mzlyAVhKhIwgXxrh5NojGIhyRckEdwpCdhgpSQgiWTRGMQNonGIGySp0SDvMDBX5KWxiB8Eo1BgE00SYJBykhNnkmSWJAcLpGaJNMgfJKyxiDAK4WNEwryhMtkJsk8CJtEYxA+icYgQIfCcgkEqcJNXhIRQdgkGoPwSTQG+e8khdu/7JOVREwQIKCwF41B2CQljUH4JLcH6SI+OUlEBQHa0SQag/BJNAbhkjxqDMIn0RgEeI4muSlID9eSkERgEKAVTaIxCJ9EYxA2ydVB8hCASVLRGAQYR5NoDMIn0RgEyFHYSGMQPonGII4kziCNvBgNJonEk4u3GAk8Sprk6eYaqbMDY0oKvUm5jfC/viGiSypV7+M3i2iDsAGpNEDYjlTa3W8RdR/r544g50ilnA0RxoZIE2NIXqQbhkAkGyKNDZHGhkhjQ6SxIdLYEGlsiDQ2JGTVeD0264U9zipPh7XOooffpA6pfNCXjxl4/c3pUzlChwzor53zwYYVfpI5pOV6LWFF/2jiJ5FDSs5jdY/0rwUAkUMeXWdBqnSqD0DikBqdqCHsjTvELm9In0IOri/0pwAEDtlSyNaRjAIAAoesKWTtuusxByBwCJp0oomwBXcYUuCQgE50ENajE4OvZAKHLB1/68Br5NqiyCGYOY8YRd77kTkEb64n7lZN+mOIX4QOwb5FX0ZVx3uOxwW+SB0CbBubemWP8/rlaaeRX+M3uUOuZENsiA25zIbYkPsZElBIHwL13U/PTjJ/cyOOEoVM3I+hziDQlELm7pPxw3eI8/7gPh1fpLA6xGnEeDDgO0UcIAzzM35HxLPIq5SXe9BLzOsj9eUaQqyXzxS1QFSfWM2cCANiHcAISJ0AnCKpUwTuIkkA3EeSInAXSQKcs1V18e24wlllUmQp9v9zXKeHi+akRAMOPVKhAqdPBZeUmnnEsO6QcJ0+4qmOSbBxFfGVRiTUqITrdKcCbyYO3/K4wX4+aQ+FfNjXhu3JfAVjjDHGGGOMMcYYY4xIPwCgfqT6TbhCLAAAAABJRU5ErkJggg==')));

enum DesktopType {
  main,
  remote,
  fileTransfer,
  viewCamera,
  terminal,
  cm,
  portForward,
}

bool isDoubleEqual(double a, double b) {
  return (a - b).abs() < _kPositionEpsilon;
}

class IconFont {
  static const _family1 = 'Tabbar';
  static const _family2 = 'PeerSearchbar';
  static const _family3 = 'AddressBook';
  static const _family4 = 'DeviceGroup';
  static const _family5 = 'More';

  IconFont._();

  static const IconData max = IconData(0xe606, fontFamily: _family1);
  static const IconData restore = IconData(0xe607, fontFamily: _family1);
  static const IconData close = IconData(0xe668, fontFamily: _family1);
  static const IconData min = IconData(0xe609, fontFamily: _family1);
  static const IconData add = IconData(0xe664, fontFamily: _family1);
  static const IconData menu = IconData(0xe628, fontFamily: _family1);
  static const IconData search = IconData(0xe6a4, fontFamily: _family2);
  static const IconData roundClose = IconData(0xe6ed, fontFamily: _family2);
  static const IconData addressBook = IconData(0xe602, fontFamily: _family3);
  static const IconData deviceGroupOutline =
      IconData(0xe623, fontFamily: _family4);
  static const IconData deviceGroupFill =
      IconData(0xe748, fontFamily: _family4);
  static const IconData more = IconData(0xe609, fontFamily: _family5);
}

extension ParseToString on ThemeMode {
  String toShortString() {
    return toString().split('.').last;
  }
}

List<Locale> supportedLocales = const [
  Locale('en', 'US'),
  Locale('zh', 'CN'),
  Locale('zh', 'TW'),
  Locale('zh', 'SG'),
  Locale('fr'),
  Locale('de'),
  Locale('it'),
  Locale('ja'),
  Locale('cs'),
  Locale('pl'),
  Locale('ko'),
  Locale('hu'),
  Locale('pt'),
  Locale('ru'),
  Locale('sk'),
  Locale('id'),
  Locale('da'),
  Locale('eo'),
  Locale('tr'),
  Locale('kz'),
  Locale('es'),
  Locale('nl'),
  Locale('nb'),
  Locale('et'),
  Locale('eu'),
  Locale('bg'),
  Locale('be'),
  Locale('vn'),
  Locale('uk'),
  Locale('fa'),
  Locale('ca'),
  Locale('el'),
  Locale('sv'),
  Locale('sq'),
  Locale('sr'),
  Locale('th'),
  Locale('sl'),
  Locale('ro'),
  Locale('lt'),
  Locale('lv'),
  Locale('ar'),
  Locale('he'),
  Locale('hr'),
];

String formatDurationToTime(Duration duration) {
  var totalTime = duration.inSeconds;
  final secs = totalTime % 60;
  totalTime = (totalTime - secs) ~/ 60;
  final mins = totalTime % 60;
  totalTime = (totalTime - mins) ~/ 60;
  return "${totalTime.toString().padLeft(2, "0")}:${mins.toString().padLeft(2, "0")}:${secs.toString().padLeft(2, "0")}";
}

const K = 1024;

const M = K * K;

const G = M * K;

String readableFileSize(double size) {
  if (size < K) {
    return "${size.toStringAsFixed(2)} B";
  } else if (size < M) {
    return "${(size / K).toStringAsFixed(2)} KB";
  } else if (size < G) {
    return "${(size / M).toStringAsFixed(2)} MB";
  } else {
    return "${(size / G).toStringAsFixed(2)} GB";
  }
}

RadioListTile<T> getRadio<T>(
    Widget title, T toValue, T curValue, ValueChanged<T?>? onChange,
    {bool? dense}) {
  return RadioListTile<T>(
    visualDensity: VisualDensity.compact,
    controlAffinity: ListTileControlAffinity.trailing,
    title: title,
    value: toValue,
    groupValue: curValue,
    onChanged: onChange,
    dense: dense,
  );
}

Map<String, String> getHttpHeaders() {
  return {
    'Authorization': 'Bearer ${bind.mainGetLocalOption(key: 'access_token')}'
  };
}

// Simple wrapper of built-in types for reference use.
class SimpleWrapper<T> {
  T value;
  SimpleWrapper(this.value);
}

int versionCmp(String v1, String v2) {
  return bind.versionToNumber(v: v1) - bind.versionToNumber(v: v2);
}

String toCapitalized(String s) {
  if (s.isEmpty) {
    return s;
  }
  return s.substring(0, 1).toUpperCase() + s.substring(1);
}

String decode_http_response(http.Response resp) {
  try {
    // https://github.com/rustdesk/rustdesk-server-pro/discussions/758
    return utf8.decode(resp.bodyBytes, allowMalformed: true);
  } catch (e) {
    debugPrint('Failed to decode response as UTF-8: $e');
    // Fallback to bodyString which handles encoding automatically
    return resp.body;
  }
}
