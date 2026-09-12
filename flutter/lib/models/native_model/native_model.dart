import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:device_info_plus/device_info_plus.dart';
import 'package:ffi/ffi.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';

import '../../common.dart';
import '../../generated_bridge.dart';
part 'platform_ffi.dart';

final class RgbaFrame extends Struct {
  @Uint32()
  external int len;
  external Pointer<Uint8> data;
}

typedef F3 = Pointer<Uint8> Function(Pointer<Utf8>, int);
typedef F3Dart = Pointer<Uint8> Function(Pointer<Utf8>, Int32);
typedef HandleEvent = Future<void> Function(Map<String, dynamic> evt);

/// The Linux bundle keeps the core library at lib/librustdesk.so next to the
/// executable. Prefer that copy, mirroring flutter/linux/main.cc: the plain
/// name relies on the loader search path, which repackaged installs may not
/// cover. https://github.com/rustdesk/rustdesk/discussions/14407
DynamicLibrary _openLinuxCoreLib() {
  final bundled =
      '${File(Platform.resolvedExecutable).parent.path}/lib/librustdesk.so';
  try {
    if (File(bundled).existsSync()) {
      return DynamicLibrary.open(bundled);
    }
  } catch (e) {
    debugPrint("Failed to load '$bundled': $e");
  }
  return DynamicLibrary.open('librustdesk.so');
}
