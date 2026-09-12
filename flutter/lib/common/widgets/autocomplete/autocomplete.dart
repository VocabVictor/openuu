import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import '../../../../models/platform_model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/peer_card.dart';
part 'all_peers_loader.dart';

@visibleForTesting
List<Peer> mergeAutocompletePeers({
  Iterable<Peer> addressBookPeers = const [],
  Iterable<Peer> groupPeers = const [],
  Iterable<Peer> lanPeers = const [],
  Iterable<Peer> recentPeers = const [],
  Iterable<String> restRecentPeerIds = const [],
}) {
  final combinedPeers = <String, Peer>{};

  void addPeer(Peer peer) {
    if (peer.id.isEmpty) {
      return;
    }
    final existingPeer = combinedPeers[peer.id];
    if (existingPeer == null) {
      combinedPeers[peer.id] = Peer.copy(peer);
    } else if (peer.online) {
      existingPeer.online = true;
    }
  }

  for (final peer in addressBookPeers) {
    addPeer(peer);
  }
  for (final peer in groupPeers) {
    addPeer(peer);
  }
  for (final peer in lanPeers) {
    addPeer(peer);
  }
  for (final peer in recentPeers) {
    addPeer(peer);
  }
  for (final id in restRecentPeerIds) {
    if (id.isNotEmpty && !combinedPeers.containsKey(id)) {
      combinedPeers[id] = Peer.fromJson({'id': id});
    }
  }

  return combinedPeers.values.toList(growable: false);
}

@visibleForTesting
bool updateAutocompletePeerOnlineStates(
  List<Peer> peers, {
  required Set<String> onlines,
  required Set<String> offlines,
}) {
  var changed = false;
  for (final peer in peers) {
    if (onlines.contains(peer.id)) {
      if (!peer.online) {
        peer.online = true;
        changed = true;
      }
    } else if (offlines.contains(peer.id)) {
      if (peer.online) {
        peer.online = false;
        changed = true;
      }
    }
  }
  return changed;
}

@visibleForTesting
List<String> autocompleteOnlineQueryIds(
  Iterable<Peer> options, {
  required int limit,
}) {
  final ids = <String>[];
  final seenIds = <String>{};
  for (final peer in options) {
    if (peer.id.isEmpty || seenIds.contains(peer.id)) {
      continue;
    }
    seenIds.add(peer.id);
    ids.add(peer.id);
    if (ids.length >= limit) {
      break;
    }
  }
  return ids;
}

class AutocompletePeerTile extends StatefulWidget {
  final VoidCallback onSelect;
  final Peer peer;

  const AutocompletePeerTile({
    Key? key,
    required this.onSelect,
    required this.peer,
  }) : super(key: key);

  @override
  AutocompletePeerTileState createState() => AutocompletePeerTileState();
}

class AutocompletePeerTileState extends State<AutocompletePeerTile> {
  List _frontN<T>(List list, int n) {
    if (list.length <= n) {
      return list;
    } else {
      return list.sublist(0, n);
    }
  }

  @override
  Widget build(BuildContext context) {
    final double tileRadius = 5;
    final name =
        '${widget.peer.username}${widget.peer.username.isNotEmpty && widget.peer.hostname.isNotEmpty ? '@' : ''}${widget.peer.hostname}';
    final greyStyle = TextStyle(
        fontSize: 11,
        color: Theme.of(context).textTheme.titleLarge?.color?.withOpacity(0.6));
    final child = GestureDetector(
        onTap: () => widget.onSelect(),
        child: Padding(
            padding: EdgeInsets.only(left: 5, right: 5),
            child: Container(
                height: 42,
                margin: EdgeInsets.only(bottom: 5),
                child: Row(
                  mainAxisSize: MainAxisSize.max,
                  children: [
                    Container(
                        decoration: BoxDecoration(
                          color: str2color(
                              '${widget.peer.id}${widget.peer.platform}', 0x7f),
                          borderRadius: BorderRadius.only(
                            topLeft: Radius.circular(tileRadius),
                            bottomLeft: Radius.circular(tileRadius),
                          ),
                        ),
                        alignment: Alignment.center,
                        width: 42,
                        height: null,
                        child: Padding(
                            padding: EdgeInsets.all(6),
                            child: getPlatformImage(widget.peer.platform,
                                size: 30))),
                    Expanded(
                      child: Container(
                          padding: EdgeInsets.only(left: 10),
                          decoration: BoxDecoration(
                            color: Theme.of(context).colorScheme.background,
                            borderRadius: BorderRadius.only(
                              topRight: Radius.circular(tileRadius),
                              bottomRight: Radius.circular(tileRadius),
                            ),
                          ),
                          child: Row(
                            children: [
                              Expanded(
                                  child: Container(
                                      margin: EdgeInsets.only(top: 2),
                                      child: Container(
                                          margin: EdgeInsets.only(top: 2),
                                          child: Column(
                                            children: [
                                              Container(
                                                  margin:
                                                      EdgeInsets.only(top: 2),
                                                  child: Row(children: [
                                                    getOnline(
                                                        8, widget.peer.online),
                                                    Expanded(
                                                        child: Text(
                                                      widget.peer.alias.isEmpty
                                                          ? formatID(
                                                              widget.peer.id)
                                                          : widget.peer.alias,
                                                      overflow:
                                                          TextOverflow.ellipsis,
                                                      style: Theme.of(context)
                                                          .textTheme
                                                          .titleSmall,
                                                    )),
                                                    widget.peer.alias.isNotEmpty
                                                        ? Padding(
                                                            padding:
                                                                const EdgeInsets
                                                                    .only(
                                                                    left: 5,
                                                                    right: 5),
                                                            child: Text(
                                                              "(${widget.peer.id})",
                                                              style: greyStyle,
                                                              overflow:
                                                                  TextOverflow
                                                                      .ellipsis,
                                                            ))
                                                        : Container(),
                                                  ])),
                                              Align(
                                                alignment: Alignment.centerLeft,
                                                child: Text(
                                                  name,
                                                  style: greyStyle,
                                                  textAlign: TextAlign.start,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                ),
                                              ),
                                            ],
                                          )))),
                            ],
                          )),
                    )
                  ],
                ))));
    final colors = _frontN(widget.peer.tags, 25)
        .map((e) => gFFI.abModel.getCurrentAbTagColor(e))
        .toList();
    return Tooltip(
      message: !(isDesktop || isWebDesktop)
          ? ''
          : widget.peer.tags.isNotEmpty
              ? '${translate('Tags')}: ${widget.peer.tags.join(', ')}'
              : '',
      child: Stack(children: [
        child,
        if (colors.isNotEmpty)
          Positioned(
            top: 5,
            right: 10,
            child: CustomPaint(
              painter: TagPainter(radius: 3, colors: colors),
            ),
          )
      ]),
    );
  }
}
