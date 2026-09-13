part of 'desktop_preview.dart';

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
