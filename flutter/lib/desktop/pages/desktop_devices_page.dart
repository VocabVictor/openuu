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
import '../widgets/device_row.dart';
import '../widgets/ui_tokens.dart';

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
  /// Favourite ids; when null the rows show no star.
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
      zh ? '电脑' : 'Computers':
          widget.peers.where((p) => !DeviceRow.mobile(p)).toList(),
      zh ? '手机/平板' : 'Phones / tablets':
          widget.peers.where(DeviceRow.mobile).toList(),
    };
    final children = <Widget>[];
    var first = true;
    for (final group in groups.entries) {
      if (!first) {
        children.add(const SizedBox(height: UiSpace.groupHeaderMarginTop));
      }
      first = false;
      final collapsed = _collapsed.contains(group.key);
      children.add(GroupHeader(
          title: group.key,
          count: group.value.length,
          collapsed: collapsed,
          onTap: () => setState(() {
                if (!_collapsed.remove(group.key)) _collapsed.add(group.key);
              })));
      if (collapsed) continue;
      children.add(const SizedBox(height: UiSpace.groupHeaderMarginBottom));
      if (group.value.isEmpty) {
        children.add(EmptyRow(text: zh ? '暂无设备' : 'No devices'));
      }
      for (var i = 0; i < group.value.length; i++) {
        if (i > 0) children.add(const SizedBox(height: UiSpace.rowCardGap));
        final peer = group.value[i];
        children.add(DeviceRow(
            peer: peer,
            local: peer.id == widget.localId,
            onOpen: widget.onOpen,
            favorite: widget.favorites?.contains(peer.id),
            onToggleFavorite: widget.onToggleFavorite));
      }
    }
    return ListView(
        padding: const EdgeInsets.fromLTRB(UiSpace.pagePaddingX, 0,
            UiSpace.pagePaddingX, UiSpace.pagePaddingBottom),
        children: children);
  }
}
