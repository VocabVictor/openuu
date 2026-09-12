part of 'peers_view.dart';

abstract class BasePeersView extends StatelessWidget {
  final PeerTabIndex peerTabIndex;
  final PeerFilter? peerFilter;
  final PeerCardBuilder peerCardBuilder;

  const BasePeersView({
    Key? key,
    required this.peerTabIndex,
    this.peerFilter,
    required this.peerCardBuilder,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    Peers peers;
    switch (peerTabIndex) {
      case PeerTabIndex.recent:
        peers = gFFI.recentPeersModel;
        break;
      case PeerTabIndex.fav:
        peers = gFFI.favoritePeersModel;
        break;
      case PeerTabIndex.lan:
        peers = gFFI.lanPeersModel;
        break;
      case PeerTabIndex.ab:
        peers = gFFI.abModel.peersModel;
        break;
      case PeerTabIndex.group:
        peers = gFFI.groupModel.peersModel;
        break;
    }
    return _PeersView(
        peers: peers,
        peerFilter: peerFilter,
        peerCardBuilder: peerCardBuilder,
        peerTabIndex: peerTabIndex);
  }
}

class RecentPeersView extends BasePeersView {
  RecentPeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          peerTabIndex: PeerTabIndex.recent,
          peerCardBuilder: (Peer peer) => RecentPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
        );

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadRecentPeers();
    return widget;
  }
}

class FavoritePeersView extends BasePeersView {
  FavoritePeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          peerTabIndex: PeerTabIndex.fav,
          peerCardBuilder: (Peer peer) => FavoritePeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
        );

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadFavPeers();
    return widget;
  }
}

class DiscoveredPeersView extends BasePeersView {
  DiscoveredPeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          peerTabIndex: PeerTabIndex.lan,
          peerCardBuilder: (Peer peer) => DiscoveredPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
        );

  @override
  Widget build(BuildContext context) {
    final widget = super.build(context);
    bind.mainLoadLanPeers();
    bind.mainDiscover();
    return widget;
  }
}

class AddressBookPeersView extends BasePeersView {
  AddressBookPeersView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          peerTabIndex: PeerTabIndex.ab,
          peerFilter: (Peer peer) =>
              _hitTag(gFFI.abModel.selectedTags, peer.tags),
          peerCardBuilder: (Peer peer) => AddressBookPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
        );

  static bool _hitTag(List<dynamic> selectedTags, List<dynamic> idents) {
    if (selectedTags.isEmpty) {
      return true;
    }
    // The result of a no-tag union with normal tags, still allows normal tags to perform union or intersection operations.
    final selectedNormalTags =
        selectedTags.where((tag) => tag != kUntagged).toList();
    if (selectedTags.contains(kUntagged)) {
      if (idents.isEmpty) return true;
      if (selectedNormalTags.isEmpty) return false;
    }
    if (gFFI.abModel.filterByIntersection.value) {
      for (final tag in selectedNormalTags) {
        if (!idents.contains(tag)) {
          return false;
        }
      }
      return true;
    } else {
      for (final tag in selectedNormalTags) {
        if (idents.contains(tag)) {
          return true;
        }
      }
      return false;
    }
  }
}

class MyGroupPeerView extends BasePeersView {
  MyGroupPeerView(
      {Key? key, EdgeInsets? menuPadding, ScrollController? scrollController})
      : super(
          key: key,
          peerTabIndex: PeerTabIndex.group,
          peerFilter: filter,
          peerCardBuilder: (Peer peer) => MyGroupPeerCard(
            peer: peer,
            menuPadding: menuPadding,
          ),
        );

  static bool filter(Peer peer) {
    final model = gFFI.groupModel;
    if (model.searchAccessibleItemNameText.isNotEmpty) {
      final text = model.searchAccessibleItemNameText.value.toLowerCase();
      final searchPeersOfUser = model.users.any((user) =>
          user.name == peer.loginName &&
          (user.name.toLowerCase().contains(text) ||
              user.displayNameOrName.toLowerCase().contains(text)));
      final searchPeersOfDeviceGroup =
          peer.device_group_name.toLowerCase().contains(text) &&
              model.deviceGroups.any((g) => g.name == peer.device_group_name);
      if (!searchPeersOfUser && !searchPeersOfDeviceGroup) {
        return false;
      }
    }
    if (model.selectedAccessibleItemName.isNotEmpty) {
      if (model.isSelectedDeviceGroup.value) {
        if (model.selectedAccessibleItemName.value != peer.device_group_name) {
          return false;
        }
      } else {
        if (model.selectedAccessibleItemName.value != peer.loginName) {
          return false;
        }
      }
    }
    return true;
  }
}
