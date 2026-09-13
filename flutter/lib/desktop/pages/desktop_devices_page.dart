import 'dart:io';
import 'package:flutter/material.dart';
import '../../common.dart';
import '../../common/widgets/login.dart';
import '../../common/widgets/peer_card.dart';
import '../../models/peer_model.dart';
import '../../models/peer_tab_model.dart';
import '../../models/platform_model.dart';
import 'desktop_device_page.dart';
import 'desktop_tab_page.dart';
import 'desktop_welcome_page.dart';

class DesktopDevicesPage extends StatefulWidget {
  const DesktopDevicesPage({super.key});
  @override
  State<DesktopDevicesPage> createState() => _DesktopDevicesPageState();
}

class _DesktopDevicesPageState extends State<DesktopDevicesPage> {
  String _localId = '';
  Set<String> _favorites = {};
  Peer? _opened;
  final _listener = 'DesktopDevicesPage';

  @override
  void initState() {
    super.initState();
    gFFI.recentPeersModel.addListener(_refresh);
    gFFI.lanPeersModel.addListener(_refresh);
    gFFI.abModel.addPeerUpdateListener(_listener, _refresh);
    gFFI.groupModel.addPeerUpdateListener(_listener, _refresh);
    bind.mainLoadRecentPeers();
    bind.mainLoadLanPeers();
    _loadLocal();
  }

  Future<void> _loadLocal() async {
    final id = await bind.mainGetMyId();
    final favorites = (await bind.mainGetFav()).toSet();
    if (mounted) {
      setState(() {
        _localId = id;
        _favorites = favorites;
      });
    }
  }

  Future<void> _toggleFavorite(Peer peer) async {
    final favorites = (await bind.mainGetFav()).toList();
    if (!favorites.remove(peer.id)) favorites.add(peer.id);
    await bind.mainStoreFav(favs: favorites);
    if (mounted) setState(() => _favorites = favorites.toSet());
  }

  void _refresh() { if (mounted) setState(() {}); }

  @override
  void dispose() {
    gFFI.recentPeersModel.removeListener(_refresh);
    gFFI.lanPeersModel.removeListener(_refresh);
    gFFI.abModel.removePeerUpdateListener(_listener);
    gFFI.groupModel.removePeerUpdateListener(_listener);
    super.dispose();
  }

  void _open(Peer peer) => setState(() => _opened = peer);

  Widget _devicePage(Peer peer) {
    void leave(VoidCallback action) { setState(() => _opened = null); action(); }
    return DesktopDevicePage(
      name: deviceName(peer), id: peer.id, online: peer.online,
      onBack: () => setState(() => _opened = null), onLogin: () => loginDialog(),
      onSettings: () => leave(() => DesktopTabPage.onAddSetting()),
      onAssistance: () => leave(() => DesktopTabPage.showHome(assistance: true)),
      onFavorites: () => leave(() => DesktopTabPage.showHome(favorites: true)),
      onConnect: () => connectInPeerTab(context, peer, PeerTabIndex.recent),
      onWatch: () => connectInPeerTab(context, peer, PeerTabIndex.recent, viewOnly: true),
      onFiles: () => connectInPeerTab(context, peer, PeerTabIndex.recent, isFileTransfer: true),
      onTerminal: () => connectInPeerTab(context, peer, PeerTabIndex.recent, isTerminal: true),
      onTunnel: () => connectInPeerTab(context, peer, PeerTabIndex.recent, isTcpTunneling: true),
      onQuickLaunch: (app) => connectInPeerTab(context, peer, PeerTabIndex.recent, quickLaunch: app),
    );
  }

  @override
  Widget build(BuildContext context) {
    final opened = _opened;
    if (opened != null) return _devicePage(opened);
    final byId = <String, Peer>{};
    for (final peer in [
      ...gFFI.abModel.allPeers(), ...gFFI.groupModel.peers,
      ...gFFI.lanPeersModel.peers, ...gFFI.recentPeersModel.peers,
    ]) {
      if (peer.id.isNotEmpty && peer.id != _localId) byId.putIfAbsent(peer.id, () => peer);
    }
    for (final id in gFFI.recentPeersModel.restPeerIds) {
      if (id.isNotEmpty && id != _localId) byId.putIfAbsent(id, () => Peer.fromJson({'id': id}));
    }
    final peers = byId.values.toList();
    if (_localId.isNotEmpty) {
      peers.insert(0, Peer.fromJson({
        'id': _localId, 'hostname': Platform.localHostname, 'platform': Platform.operatingSystem,
      }));
    }
    return DesktopWelcomePage(
      onLogin: () => loginDialog(), onDevices: () {},
      onSettings: () => DesktopTabPage.onAddSetting(),
      onAssistance: () => DesktopTabPage.showHome(assistance: true),
      onFavorites: () => DesktopTabPage.showHome(favorites: true),
      content: DeviceGroups(
          peers: peers,
          localId: _localId,
          onOpen: _open,
          favorites: _favorites,
          onToggleFavorite: _toggleFavorite),
    );
  }
}

String deviceName(Peer peer) => peer.alias.isNotEmpty ? peer.alias
  : peer.hostname.isNotEmpty ? peer.hostname : peer.id;

class DeviceGroups extends StatefulWidget {
  final List<Peer> peers;
  final String localId;
  final ValueChanged<Peer> onOpen;
  /// Favourite ids; when null the cards show no star.
  final Set<String>? favorites;
  final ValueChanged<Peer>? onToggleFavorite;
  const DeviceGroups(
      {super.key,
      required this.peers,
      required this.localId,
      required this.onOpen,
      this.favorites,
      this.onToggleFavorite});
  @override
  State<DeviceGroups> createState() => _DeviceGroupsState();
}

class _DeviceGroupsState extends State<DeviceGroups> {
  final _collapsed = <String>{};
  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final groups = {
      zh ? '电脑' : 'Computers': widget.peers.where((p) => !DeviceCard.mobile(p)).toList(),
      zh ? '手机/平板' : 'Phones / tablets': widget.peers.where(DeviceCard.mobile).toList(),
    };
    return LayoutBuilder(builder: (context, bounds) => ListView(
      padding: EdgeInsets.symmetric(horizontal: (bounds.maxWidth * .05).clamp(20.0, 48.0), vertical: 12),
      children: [for (final group in groups.entries) ...[
        TextButton(onPressed: () => setState(() {
          if (!_collapsed.remove(group.key)) _collapsed.add(group.key);
        }), style: TextButton.styleFrom(foregroundColor: const Color(0xff20262d), alignment: Alignment.centerLeft),
          child: Row(children: [Icon(_collapsed.contains(group.key) ? Icons.chevron_right : Icons.expand_more, size: 20),
            const SizedBox(width: 10), Text('${group.key} ${group.value.length}', style: const TextStyle(fontSize: 16))])),
        if (!_collapsed.contains(group.key)) ...[
          if (group.value.isEmpty) Padding(padding: const EdgeInsets.all(16),
            child: Text(zh ? '暂无设备' : 'No devices', style: const TextStyle(color: Colors.grey))),
          for (final peer in group.value)
            DeviceCard(
                peer: peer,
                local: peer.id == widget.localId,
                onOpen: widget.onOpen,
                favorite: widget.favorites?.contains(peer.id),
                onToggleFavorite: widget.onToggleFavorite),
        ],
        const SizedBox(height: 18),
      ]],
    ));
  }
}

/// One device row, shared by the grouped overview and other device lists.
class DeviceCard extends StatelessWidget {
  final Peer peer;
  final bool local;
  final ValueChanged<Peer> onOpen;
  /// Whether the peer is in the favourites; null hides the star.
  final bool? favorite;
  final ValueChanged<Peer>? onToggleFavorite;
  const DeviceCard(
      {super.key,
      required this.peer,
      required this.local,
      required this.onOpen,
      this.favorite,
      this.onToggleFavorite});

  static bool mobile(Peer p) => ['android', 'ios', 'ipados'].contains(p.platform.toLowerCase());

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    return Padding(padding: const EdgeInsets.only(bottom: 5), child: Material(
      color: Colors.white,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(4), side: const BorderSide(color: Color(0xffdfe3e6))),
      child: ListTile(
        contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        leading: Container(width: 36, height: 36,
          decoration: BoxDecoration(color: const Color(0xff3979ff), borderRadius: BorderRadius.circular(5)),
          child: Icon(mobile(peer) ? Icons.phone_android : Icons.desktop_windows_outlined, color: Colors.white, size: 24)),
        title: Row(children: [Flexible(child: Text(deviceName(peer), maxLines: 1, overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontSize: 16, color: Color(0xff20262d)))),
          if (local) Container(margin: const EdgeInsets.only(left: 10), padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
            decoration: BoxDecoration(color: const Color(0xffe5f1ff), borderRadius: BorderRadius.circular(3)),
            child: Text(zh ? '本机' : 'This device', style: const TextStyle(fontSize: 12, color: Color(0xff3979ff)))),
        ]),
        trailing: Row(mainAxisSize: MainAxisSize.min, children: [
          if (favorite != null && !local)
            IconButton(
              tooltip: favorite! ? (zh ? '取消收藏' : 'Remove from favourites') : (zh ? '收藏' : 'Add to favourites'),
              icon: Icon(favorite! ? Icons.star : Icons.star_border, size: 20,
                color: favorite! ? const Color(0xfff5a623) : const Color(0xff9aa3ad)),
              onPressed: onToggleFavorite == null ? null : () => onToggleFavorite!(peer)),
          SizedBox(width: 24, child: local ? null : const Icon(Icons.chevron_right, size: 20)),
        ]),
        onTap: local ? null : () => onOpen(peer),
      ),
    ));
  }
}
