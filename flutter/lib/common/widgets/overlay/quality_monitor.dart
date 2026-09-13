part of 'overlay.dart';

/// Shown where a number is not available, rather than a stale or invented one.
const _kNoValue = '—';

class QualityMonitor extends StatelessWidget {
  final QualityMonitorModel qualityMonitorModel;
  QualityMonitor(this.qualityMonitorModel);

  Widget _row(String info, String? value, {Color? rightColor}) {
    return Row(
      children: [
        Expanded(
            flex: 8,
            child: AutoSizeText(info,
                style: TextStyle(color: Color.fromARGB(255, 210, 210, 210)),
                textAlign: TextAlign.right,
                maxLines: 1)),
        Spacer(flex: 1),
        Expanded(
            flex: 8,
            child: AutoSizeText(value ?? _kNoValue,
                style: TextStyle(color: rightColor ?? Colors.white),
                maxLines: 1)),
      ],
    );
  }

  /// "Direct (TCP)" or "Relay (TCP)": which path the session actually took,
  /// the one thing about a slow session that is not visible anywhere else.
  String _connection() {
    final model = qualityMonitorModel.parent.target?.ffiModel;
    final direct = model?.direct;
    if (direct == null) {
      return _kNoValue;
    }
    final label = translate(direct ? 'Direct' : 'Relayed');
    var streamType = model?.cachedPeerData.streamType ?? '';
    if (streamType == 'Relay') {
      streamType = 'TCP';
    }
    return streamType.isEmpty ? label : '$label ($streamType)';
  }

  /// No frames, no meaningful delay: the last one measured belongs to a
  /// frame that is no longer on screen.
  bool get _idle {
    final fps = qualityMonitorModel.data.fps;
    return fps == null || fps.replaceAll(' ', '').replaceAll('0', '').isEmpty;
  }

  @override
  Widget build(BuildContext context) => ChangeNotifierProvider.value(
      value: qualityMonitorModel,
      child: Consumer<QualityMonitorModel>(
          builder: (context, qualityMonitorModel, child) {
        if (!qualityMonitorModel.show) {
          return const SizedBox.shrink();
        }
        final data = qualityMonitorModel.data;
        return Container(
          constraints: BoxConstraints(maxWidth: 200),
          padding: const EdgeInsets.all(8),
          color: MyTheme.canvasColor.withAlpha(150),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _row(translate('Connection'), _connection()),
              _row("Speed", data.speed),
              _row("FPS", data.fps),
              _row("Delay",
                  _idle || data.delay == null ? null : '${data.delay}ms',
                  rightColor: Colors.green),
              _row("Target Bitrate",
                  data.targetBitrate == null ? null : '${data.targetBitrate}kb'),
              _row("Codec", data.codecFormat),
              _row("Chroma", data.chroma),
              // Packet loss is not observable on a TCP or relayed session,
              // and the KCP layer exposes no retransmission counter yet; the
              // tooltip says so rather than leaving an unexplained dash.
              Tooltip(
                  message: translate('packet-loss-unavailable-tip'),
                  child: _row(translate('Packet loss'), null)),
            ],
          ),
        );
      }));
}
