import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class DeviceActionBar extends StatelessWidget {
  final String id;
  final VoidCallback onFiles, onWatch, onTerminal, onTunnel;

  const DeviceActionBar({super.key, required this.id, required this.onFiles,
    required this.onWatch, required this.onTerminal, required this.onTunnel});

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    return LayoutBuilder(builder: (context, constraints) {
      final compact = constraints.maxWidth < 560;
      Widget action(String label, IconData icon, VoidCallback callback) => Expanded(
        child: Tooltip(message: label, child: TextButton(
          onPressed: callback,
          style: TextButton.styleFrom(
            foregroundColor: const Color(0xff243747),
            minimumSize: const Size(0, 52),
            padding: const EdgeInsets.symmetric(horizontal: 8),
            shape: const RoundedRectangleBorder(),
          ),
          child: compact ? Icon(icon, size: 19) : FittedBox(fit: BoxFit.scaleDown,
            child: Row(mainAxisSize: MainAxisSize.min, children: [
              Icon(icon, size: 19), const SizedBox(width: 8), Text(label),
            ])),
        )),
      );
      Widget divider() => const SizedBox(height: 22,
        child: VerticalDivider(width: 1, thickness: 1, color: Color(0xffd9dfe5)));
      return Container(
        height: 52,
        decoration: const BoxDecoration(gradient: LinearGradient(
          begin: Alignment.topCenter, end: Alignment.bottomCenter,
          colors: [Color(0xffedf0f7), Color(0xfffafcfe)],
        )),
        child: Row(children: [
          action(t('文件传输', 'Files'), Icons.folder_open_outlined, onFiles), divider(),
          action(t('观看模式', 'View only'), Icons.ondemand_video, onWatch), divider(),
          action(t('终端', 'Terminal'), Icons.terminal, onTerminal), divider(),
          action(t('端口映射', 'Port forwarding'), Icons.settings_ethernet, onTunnel), divider(),
          SizedBox(width: 52, height: 52, child: PopupMenuButton<String>(
            tooltip: t('更多', 'More'),
            icon: const Icon(Icons.grid_view_outlined, size: 20, color: Color(0xff243747)),
            position: PopupMenuPosition.under,
            onSelected: (_) async {
              await Clipboard.setData(ClipboardData(text: id));
            },
            itemBuilder: (_) => [PopupMenuItem(value: 'copy-id',
              child: Text(t('复制设备 ID', 'Copy device ID')))],
          )),
        ]),
      );
    });
  }
}
