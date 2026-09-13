import 'package:flutter/material.dart';
import '../../common.dart';
import '../../common/widgets/login.dart';
import '../../common/widgets/peer_card.dart';
import '../../models/peer_model.dart';
import '../../models/peer_tab_model.dart';
import '../../models/platform_model.dart';
import 'desktop_device_page.dart';
import 'desktop_devices_page.dart';
import 'desktop_tab_page.dart';
import 'desktop_welcome_page.dart';
import '../widgets/device_row.dart';
import '../widgets/ui_tokens.dart';

/// 「收藏设备」: the favourite ids, shown with the same grouped cards as the
/// all-devices page inside the signed-in home shell.
class DesktopFavoritesPage extends StatefulWidget {
  const DesktopFavoritesPage({super.key});
  @override
  State<DesktopFavoritesPage> createState() => _DesktopFavoritesPageState();
}

class _DesktopFavoritesPageState extends State<DesktopFavoritesPage> {
  List<String> _favorites = [];
  String _localId = '';
  Peer? _opened;
  final _listener = 'DesktopFavoritesPage';

  @override
  void initState() {
    super.initState();
    gFFI.recentPeersModel.addListener(_refresh);
    gFFI.lanPeersModel.addListener(_refresh);
    gFFI.abModel.addPeerUpdateListener(_listener, _refresh);
    gFFI.groupModel.addPeerUpdateListener(_listener, _refresh);
    bind.mainLoadRecentPeers();
    bind.mainLoadLanPeers();
    _load();
  }

  Future<void> _load() async {
    final favorites = await bind.mainGetFav();
    final id = await bind.mainGetMyId();
    if (mounted) {
      setState(() {
        _favorites = favorites;
        _localId = id;
      });
    }
  }

  Future<void> _toggleFavorite(Peer peer) async {
    final favorites = (await bind.mainGetFav()).toList();
    if (!favorites.remove(peer.id)) favorites.add(peer.id);
    await bind.mainStoreFav(favs: favorites);
    if (mounted) setState(() => _favorites = favorites);
  }

  void _refresh() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    gFFI.recentPeersModel.removeListener(_refresh);
    gFFI.lanPeersModel.removeListener(_refresh);
    gFFI.abModel.removePeerUpdateListener(_listener);
    gFFI.groupModel.removePeerUpdateListener(_listener);
    super.dispose();
  }

  Widget _devicePage(Peer peer) {
    void leave(VoidCallback action) {
      setState(() => _opened = null);
      action();
    }

    return DesktopDevicePage(
      name: deviceName(peer),
      id: peer.id,
      online: peer.online,
      onBack: () => setState(() => _opened = null),
      onLogin: () => loginDialog(),
      onSettings: () => leave(() => DesktopTabPage.onAddSetting()),
      onAssistance: () => leave(() => DesktopTabPage.showHome(assistance: true)),
      onFavorites: () => setState(() => _opened = null),
      onConnect: () => connectInPeerTab(context, peer, PeerTabIndex.fav),
      onWatch: () =>
          connectInPeerTab(context, peer, PeerTabIndex.fav, viewOnly: true),
      onFiles: () => connectInPeerTab(context, peer, PeerTabIndex.fav,
          isFileTransfer: true),
      onTerminal: () =>
          connectInPeerTab(context, peer, PeerTabIndex.fav, isTerminal: true),
      onTunnel: () => connectInPeerTab(context, peer, PeerTabIndex.fav,
          isTcpTunneling: true),
      onQuickLaunch: (app) =>
          connectInPeerTab(context, peer, PeerTabIndex.fav, quickLaunch: app),
    );
  }

  @override
  Widget build(BuildContext context) {
    final opened = _opened;
    if (opened != null) return _devicePage(opened);
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final byId = <String, Peer>{};
    for (final peer in [
      ...gFFI.abModel.allPeers(),
      ...gFFI.groupModel.peers,
      ...gFFI.lanPeersModel.peers,
      ...gFFI.recentPeersModel.peers,
    ]) {
      if (peer.id.isNotEmpty) byId.putIfAbsent(peer.id, () => peer);
    }
    final peers = [
      for (final id in _favorites)
        if (id.isNotEmpty) byId[id] ?? Peer.fromJson({'id': id}),
    ];
    return DesktopWelcomePage(
      favoritesSelected: true,
      onLogin: () => loginDialog(),
      onDevices: () => DesktopTabPage.showHome(),
      onSettings: () => DesktopTabPage.onAddSetting(),
      onAssistance: () => DesktopTabPage.showHome(assistance: true),
      onFavorites: () {},
      content: peers.isEmpty
          ? Padding(
              padding: const EdgeInsets.symmetric(
                  horizontal: UiSpace.pagePaddingX),
              child: Column(children: [
                EmptyRow(
                    text: zh
                        ? '暂无收藏设备，在设备卡片上点击星标后会显示在这里。'
                        : 'No favourites yet. Star a device to see it here.'),
              ]))
          : DeviceGroups(
              peers: peers,
              localId: _localId,
              onOpen: _open,
              favorites: _favorites.toSet(),
              onToggleFavorite: _toggleFavorite),
    );
  }

  void _open(Peer peer) => setState(() => _opened = peer);
}
