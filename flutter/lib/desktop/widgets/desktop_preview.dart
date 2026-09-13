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

import '../../models/platform_model.dart';
import 'ui_tokens.dart';

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

/// Captures only the already-authorized rendered desktop, without a new connection.
class DesktopPreviewCapture extends StatefulWidget {
  final String peer;
  final bool Function() ready;
  final Widget child;
  const DesktopPreviewCapture(
      {super.key,
      required this.peer,
      required this.ready,
      required this.child});

  @override
  State<DesktopPreviewCapture> createState() => _DesktopPreviewCaptureState();
}

class _DesktopPreviewCaptureState extends State<DesktopPreviewCapture> {
  final _boundary = GlobalKey();
  Timer? _timer;
  bool _busy = false;
  DateTime? _lastCapture;
  String? _key;

  @override
  void initState() {
    super.initState();
    _key = DesktopPreview.cacheKey(widget.peer);
    _timer = Timer.periodic(const Duration(seconds: 2), (_) => _capture());
  }

  Future<void> _capture() async {
    if (_busy ||
        !mounted ||
        !widget.ready() ||
        _key == null ||
        DesktopPreview.cacheKey(widget.peer) != _key ||
        (_lastCapture != null &&
            DateTime.now().difference(_lastCapture!).inSeconds < 30)) return;
    final boundary = _boundary.currentContext?.findRenderObject();
    // Not debugNeedsPaint: it assigns its `late bool` inside an assert, so in
    // a release build reading it throws LateInitializationError. That threw on
    // every attempt here, was swallowed by the catch below, and no preview was
    // ever written outside a debug build. toImage tolerates a boundary that
    // still needs paint, so the check is simply gone.
    if (boundary is! RenderRepaintBoundary ||
        !boundary.attached ||
        boundary.size.isEmpty) return;
    _busy = true;
    ui.Image? image;
    try {
      image = await boundary.toImage(
          pixelRatio: math.min(1.0, 960 / boundary.size.longestSide));
      final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
      if (bytes != null &&
          mounted &&
          widget.ready() &&
          DesktopPreview.cacheKey(widget.peer) == _key) {
        await DesktopPreview.save(_key!,
            bytes.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes));
        _lastCapture = DateTime.now();
      }
    } catch (e) {
      debugPrint('Desktop preview capture failed: $e');
      _lastCapture = DateTime.now();
    } finally {
      image?.dispose();
      _busy = false;
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      RepaintBoundary(key: _boundary, child: widget.child);
}

/// Caps the 16:9 panel so a wide window cannot make it ~660 high and push the
/// sections under it off a 1080p screen.
const double _kMaxPreviewHeight = 360;
const double _kEmptyIconSize = 48;

class DesktopPreviewPanel extends StatefulWidget {
  final String peer;
  final VoidCallback onConnect;
  final Future<DesktopPreview?> Function()? loadPreview;
  const DesktopPreviewPanel(
      {super.key,
      required this.peer,
      required this.onConnect,
      this.loadPreview});

  @override
  State<DesktopPreviewPanel> createState() => _DesktopPreviewPanelState();
}

class _DesktopPreviewPanelState extends State<DesktopPreviewPanel> {
  DesktopPreview? _preview;
  bool _hover = false;
  String? _error;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _load();
    _timer = Timer.periodic(const Duration(seconds: 5), (_) => _load());
  }

  Future<void> _load() async {
    try {
      final loader = widget.loadPreview;
      final key = loader == null ? DesktopPreview.cacheKey(widget.peer) : null;
      final preview = loader != null
          ? await loader()
          : key == null
              ? null
              : await DesktopPreview.load(key);
      if (mounted &&
          (loader != null || key == DesktopPreview.cacheKey(widget.peer))) {
        setState(() {
          _preview = preview;
          _error = null;
        });
      }
    } catch (e) {
      debugPrint('Desktop preview read failed: $e');
      if (mounted) {
        setState(() {
          _preview = null;
          _error = 'error';
        });
      }
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final preview = _preview;
    return ConstrainedBox(
        constraints: const BoxConstraints(maxHeight: _kMaxPreviewHeight),
        child: AspectRatio(
        aspectRatio: 16 / 9,
        child: MouseRegion(
            onEnter: (_) => setState(() => _hover = true),
            onExit: (_) => setState(() => _hover = false),
            child: Stack(fit: StackFit.expand, children: [
              const DecoratedBox(
                  decoration: BoxDecoration(
                      gradient: LinearGradient(
                          begin: Alignment.topLeft,
                          end: Alignment.bottomRight,
                          colors: [Color(0xffdcecf8), Color(0xff729fc2)]))),
              if (preview != null)
                Image.memory(preview.bytes,
                    fit: BoxFit.contain,
                    gaplessPlayback: true,
                    errorBuilder: (_, __, ___) => const Center(
                        child: Icon(Icons.broken_image_outlined, size: 40))),
              Material(
                  color: Colors.transparent,
                  child: InkWell(
                      onTap: widget.onConnect,
                      child: AnimatedContainer(
                          duration: const Duration(milliseconds: 150),
                          color: Colors.black.withOpacity(_hover ? .24 : .10),
                          child: Center(
                              child: Column(
                                  mainAxisSize: MainAxisSize.min,
                                  children: [
                                if (preview == null)
                                  const Icon(Icons.desktop_windows_outlined,
                                      size: _kEmptyIconSize,
                                      color: Colors.white),
                                const SizedBox(height: UiSpace.s3),
                                Text(zh ? '进入桌面  →' : 'Enter desktop  →',
                                    style: UiType.pageTitle
                                        .copyWith(color: Colors.white)),
                                if (preview == null)
                                  Padding(
                                      padding: const EdgeInsets.only(
                                          top: UiSpace.s2),
                                      child: Text(
                                          _error != null
                                              ? (zh
                                                  ? '预览读取失败，请重试'
                                                  : 'Unable to load preview')
                                              : (zh
                                                  ? '连接后将保存最近桌面画面'
                                                  : 'A preview will be saved after connecting'),
                                          style: UiType.caption.copyWith(
                                              color: Colors.white))),
                              ]))))),
              if (preview != null)
                Positioned(
                    left: UiSpace.s3,
                    bottom: UiSpace.s3,
                    child: Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: UiSpace.tagPaddingX, vertical: 5),
                        decoration: BoxDecoration(
                            color: Colors.black45,
                            borderRadius:
                                BorderRadius.circular(UiSpace.tagRadius)),
                        child: Text(
                            '${zh ? '最近画面 · 非实时' : 'Saved preview · Not live'}  ${MaterialLocalizations.of(context).formatShortDate(preview.capturedAt.toLocal())} ${MaterialLocalizations.of(context).formatTimeOfDay(TimeOfDay.fromDateTime(preview.capturedAt.toLocal()))}',
                            style: UiType.tag.copyWith(color: Colors.white)))),
              Positioned(
                  right: 8,
                  top: 8,
                  child: IconButton(
                      tooltip: zh ? '重新读取预览缓存' : 'Reload saved preview',
                      onPressed: _load,
                      icon: const Icon(Icons.refresh, color: Colors.white))),
            ]))));
  }
}
