part of 'overlay.dart';

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
            child: AutoSizeText(value ?? '',
                style: TextStyle(color: rightColor ?? Colors.white),
                maxLines: 1)),
      ],
    );
  }

  @override
  Widget build(BuildContext context) => ChangeNotifierProvider.value(
      value: qualityMonitorModel,
      child: Consumer<QualityMonitorModel>(
          builder: (context, qualityMonitorModel, child) => qualityMonitorModel
                  .show
              ? Container(
                  constraints: BoxConstraints(maxWidth: 200),
                  padding: const EdgeInsets.all(8),
                  color: MyTheme.canvasColor.withAlpha(150),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      _row("Speed", qualityMonitorModel.data.speed ?? '-'),
                      _row("FPS", qualityMonitorModel.data.fps ?? '-'),
                      // let delay be 0 if fps is 0
                      _row(
                          "Delay",
                          "${qualityMonitorModel.data.delay == null ? '-' : (qualityMonitorModel.data.fps ?? "").replaceAll(' ', '').replaceAll('0', '').isEmpty ? 0 : qualityMonitorModel.data.delay}ms",
                          rightColor: Colors.green),
                      _row("Target Bitrate",
                          "${qualityMonitorModel.data.targetBitrate ?? '-'}kb"),
                      _row(
                          "Codec", qualityMonitorModel.data.codecFormat ?? '-'),
                      _row("Chroma", qualityMonitorModel.data.chroma ?? '-'),
                    ],
                  ),
                )
              : const SizedBox.shrink()));
}
