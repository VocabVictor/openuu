import 'package:flutter/material.dart';
import '../../common.dart';
import '../../models/online_poller.dart';
import '../../models/online_presence.dart';
import 'desktop_welcome_page.dart';
import '../widgets/desktop_preview.dart';
import '../widgets/quick_launch.dart';
import '../widgets/device_action_bar.dart';

/// Device details use the existing peer connection actions supplied by the caller.
class DesktopDevicePage extends StatefulWidget {
  final void Function(String)? onQuickLaunch;
  final String name;
  final String id;
  final VoidCallback onBack, onLogin, onSettings, onAssistance, onFavorites;
  final VoidCallback onConnect, onWatch, onFiles, onTerminal, onTunnel;

  const DesktopDevicePage({super.key, this.onQuickLaunch, required this.name, required this.id,
    required this.onBack, required this.onLogin,
    required this.onSettings, required this.onAssistance, required this.onFavorites,
    required this.onConnect, required this.onWatch, required this.onFiles, required this.onTerminal,
    required this.onTunnel});

  @override
  State<DesktopDevicePage> createState() => _DesktopDevicePageState();
}

class _DesktopDevicePageState extends State<DesktopDevicePage> {
  late final OnlinePoller _poller;

  @override
  void initState() {
    super.initState();
    // Nobody else asks the server about this device: the batch query lives in
    // the legacy peers view, which this page does not mount.
    _poller = OnlinePoller(
        name: 'DesktopDevicePage',
        onChanged: () {
          if (mounted) setState(() {});
        })
      ..start()
      ..watch([widget.id]);
  }

  @override
  void didUpdateWidget(DesktopDevicePage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.id != widget.id) {
      _poller.watch([widget.id]);
    }
  }

  @override
  void dispose() {
    _poller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    final name = widget.name;
    final id = widget.id;
    final onQuickLaunch = widget.onQuickLaunch;
    final presence = _poller.presenceOf(id);
    final online = presence == PeerPresence.online;
    // Three states, three sentences: "not asked yet" is not "offline".
    final status = translate(switch (presence) {
      PeerPresence.online => 'Online',
      PeerPresence.offline => 'Offline',
      PeerPresence.unknown => 'Status unknown',
    });
    return DesktopWelcomePage(
      onLogin: widget.onLogin, onSettings: widget.onSettings, onDevices: widget.onBack,
      onAssistance: widget.onAssistance, onFavorites: widget.onFavorites,
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
                DesktopPreviewPanel(peer: id, onConnect: widget.onConnect),
                DeviceActionBar(id: id, onFiles: widget.onFiles, onWatch: widget.onWatch,
                  onTerminal: widget.onTerminal, onTunnel: widget.onTunnel),
              ])),
            const SizedBox(height: 22),
            Text(t('快速启动', 'Quick launch'), style: const TextStyle(fontSize: 16)),
            const SizedBox(height: 10),
            if (onQuickLaunch != null) QuickLaunchPanel(peer: id, onOpen: onQuickLaunch),
          ]));
      }),
    );
  }

}
