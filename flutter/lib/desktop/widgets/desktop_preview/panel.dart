part of 'desktop_preview.dart';

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
                                Text('${translate('Enter desktop')}  →',
                                    style: UiType.pageTitle
                                        .copyWith(color: Colors.white)),
                                if (preview == null)
                                  Padding(
                                      padding: const EdgeInsets.only(
                                          top: UiSpace.s2),
                                      child: Text(
                                          _error != null
                                              ? translate(
                                                  'Unable to load preview')
                                              : translate(
                                                  'A preview will be saved after connecting'),
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
                            '${translate('Saved preview · Not live')}  ${MaterialLocalizations.of(context).formatShortDate(preview.capturedAt.toLocal())} ${MaterialLocalizations.of(context).formatTimeOfDay(TimeOfDay.fromDateTime(preview.capturedAt.toLocal()))}',
                            style: UiType.tag.copyWith(color: Colors.white)))),
              Positioned(
                  right: 8,
                  top: 8,
                  child: IconButton(
                      tooltip: translate('Reload saved preview'),
                      onPressed: _load,
                      icon: const Icon(Icons.refresh, color: Colors.white))),
            ]))));
  }
}
