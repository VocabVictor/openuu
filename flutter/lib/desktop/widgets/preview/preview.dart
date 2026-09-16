import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:path_provider/path_provider.dart';
import 'package:uuid/uuid.dart';

import '../../../common.dart' show translate;
import '../../../models/platform_model.dart';
import '../ui_tokens.dart';

part 'capture.dart';
part 'panel.dart';

class DesktopPreview {
  final Uint8List bytes;
  final DateTime capturedAt;
  DesktopPreview(this.bytes, this.capturedAt);

  // A new login cannot expose screenshots from another login or server.
  static String? cacheKey(String peer) {
    final token = bind.mainGetLocalOption(key: 'access_token');
    return keyFor(bind.mainGetOptionSync(key: 'api-server'), token, peer);
  }

  static String? keyFor(String server, String token, String peer) {
    if (token.isEmpty || peer.isEmpty) return null;
    return const Uuid().v5(
        Uuid.NAMESPACE_URL,
        jsonEncode([
          server,
          token,
          peer,
        ]));
  }

  static Future<File> _file(String key) async {
    final root = await getApplicationSupportDirectory();
    final dir = Directory('${root.path}/desktop-previews');
    await dir.create(recursive: true);
    return File('${dir.path}/$key.png');
  }

  static Future<DesktopPreview?> load(String key) async {
    final file = await _file(key);
    if (!await file.exists()) return null;
    final stat = await file.stat();
    if (DateTime.now().difference(stat.modified).inDays >= 7) {
      await file.delete();
      return null;
    }
    return DesktopPreview(await file.readAsBytes(), stat.modified);
  }

  static Future<void> save(String key, Uint8List bytes) async {
    final file = await _file(key);
    // Unique temporary files also isolate simultaneous remote windows.
    final temp = File('${file.path}.${const Uuid().v4()}.tmp');
    try {
      await temp.writeAsBytes(bytes, flush: true);
      await temp.rename(file.path);
    } finally {
      if (await temp.exists()) await temp.delete();
    }
    await for (final entry in file.parent.list()) {
      if (entry is File &&
          DateTime.now().difference((await entry.stat()).modified).inDays >=
              7) {
        await entry.delete();
      }
    }
  }
}
