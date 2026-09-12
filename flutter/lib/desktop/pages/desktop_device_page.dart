import 'package:flutter/material.dart';
import 'desktop_welcome_page.dart';
import '../widgets/quick_launch.dart';
import '../widgets/device_action_bar.dart';

/// Device details use the existing peer connection actions supplied by the caller.
class DesktopDevicePage extends StatelessWidget {
  final void Function(String)? onQuickLaunch;
  final String name;
  final String id;
  final bool online;
  final VoidCallback onBack, onLogin, onSettings, onAssistance, onFavorites;
  final VoidCallback onConnect, onWatch, onFiles, onTerminal, onTunnel;

  const DesktopDevicePage({super.key, this.onQuickLaunch, required this.name, required this.id,
    required this.online, required this.onBack, required this.onLogin,
    required this.onSettings, required this.onAssistance, required this.onFavorites,
    required this.onConnect, required this.onWatch, required this.onFiles, required this.onTerminal,
    required this.onTunnel});

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    final status = online ? t('在线', 'Online') : t('离线或状态未知', 'Offline or unknown');
    return DesktopWelcomePage(
      onLogin: onLogin, onSettings: onSettings, onDevices: onBack,
      onAssistance: onAssistance, onFavorites: onFavorites,
      deviceItem: Container(
        margin: const EdgeInsets.symmetric(horizontal: 6),
        decoration: BoxDecoration(color: const Color(0xffe2e8ec), borderRadius: BorderRadius.circular(5)),
        child: ListTile(dense: true,
          leading: Icon(Icons.desktop_windows_outlined, color: DesktopWelcomePage.blue, size: 21),
          title: Text(name, maxLines: 1, overflow: TextOverflow.ellipsis),
          subtitle: Text(status, style: const TextStyle(fontSize: 11)),
          selected: true, onTap: () {})),
      header: Row(children: [
        Container(padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
          decoration: BoxDecoration(color: const Color(0xff20262d), borderRadius: BorderRadius.circular(20)),
          child: Row(mainAxisSize: MainAxisSize.min, children: [
            Icon(Icons.circle, size: 8, color: online ? const Color(0xff18dfa0) : Colors.grey),
            const SizedBox(width: 5), Text(status, style: const TextStyle(color: Colors.white, fontSize: 12))])),
        const SizedBox(width: 12),
        Expanded(child: Text(name, maxLines: 1, overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontSize: 24, fontWeight: FontWeight.w600))),
        Tooltip(message: 'ID: $id', child: const Icon(Icons.info_outline, size: 20)),
      ]),
      content: LayoutBuilder(builder: (context, bounds) {
        final inset = (bounds.maxWidth * .05).clamp(20.0, 48.0);
        return SingleChildScrollView(padding: EdgeInsets.fromLTRB(inset, 20, inset, 28),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Container(clipBehavior: Clip.antiAlias,
              decoration: BoxDecoration(color: Colors.white, border: Border.all(color: const Color(0xffdce2e7)), borderRadius: BorderRadius.circular(6)),
              child: Column(children: [
                AspectRatio(aspectRatio: 2.5, child: Material(
                  color: const Color(0xffe5eff8), child: InkWell(onTap: onConnect,
                    child: Center(child: Column(mainAxisSize: MainAxisSize.min, children: [
                      const Icon(Icons.desktop_windows_outlined, size: 52, color: Color(0xff7097bc)),
                      const SizedBox(height: 14),
                      Text(t('进入桌面  →', 'Enter desktop  →'), style: const TextStyle(fontSize: 20, color: Color(0xff24476b))),
                      const SizedBox(height: 8),
                      Text(t('此设备尚未提供桌面预览', 'Desktop preview is not available'), style: const TextStyle(fontSize: 12, color: Color(0xff697c8e))),
                    ]))))),
                DeviceActionBar(id: id, onFiles: onFiles, onWatch: onWatch,
                  onTerminal: onTerminal, onTunnel: onTunnel),
              ])),
            const SizedBox(height: 22),
            Text(t('快速启动', 'Quick launch'), style: const TextStyle(fontSize: 16)),
            const SizedBox(height: 10),
            if (onQuickLaunch != null) QuickLaunchPanel(peer: id, onOpen: onQuickLaunch!),
          ]));
      }),
    );
  }

}
