import 'package:flutter/material.dart';
import '../../common.dart';
import '../../models/online_presence.dart';
import '../../models/peer_model.dart';
import '../pages/desktop_devices_page.dart' show deviceName;
import 'ui_tokens.dart';

/// A collapsible group title: chevron and text share the page's left edge
/// with the cards below (no extra indent); the count is lighter.
class GroupHeader extends StatelessWidget {
  final String title;
  final int count;
  final bool collapsed;
  final VoidCallback onTap;
  const GroupHeader(
      {super.key,
      required this.title,
      required this.count,
      required this.collapsed,
      required this.onTap});

  @override
  Widget build(BuildContext context) => InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(UiSpace.s1),
      child: SizedBox(
          height: UiSpace.groupHeaderHeight,
          child: Row(children: [
            Icon(collapsed ? Icons.chevron_right : Icons.expand_more,
                size: UiSpace.groupChevronSize, color: UiColor.muted),
            const SizedBox(width: UiSpace.groupChevronGap),
            Text(title, style: UiType.groupTitle),
            const SizedBox(width: UiSpace.groupCountGap),
            Text('$count', style: UiType.groupCount),
          ])));
}

/// The empty state of a group: a row-high placeholder on the card's edge.
class EmptyRow extends StatelessWidget {
  final String text;
  const EmptyRow({super.key, required this.text});

  @override
  Widget build(BuildContext context) => Container(
      height: UiSpace.emptyStateHeight,
      alignment: Alignment.centerLeft,
      padding: const EdgeInsets.symmetric(horizontal: UiSpace.rowCardPaddingX),
      decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(UiSpace.rowCardRadius),
          border: Border.all(color: UiColor.border)),
      child: Text(text, style: UiType.caption));
}

/// Whether a device is up, drawn so the three states are told apart without
/// relying on the colour: filled for up, a ring for down, a dash for not
/// answered yet (AGENTS.md). The word itself is in the tooltip.
class PresenceDot extends StatelessWidget {
  final PeerPresence presence;
  const PresenceDot({super.key, required this.presence});

  @override
  Widget build(BuildContext context) {
    const size = UiSpace.statusDotSize;
    final Widget mark;
    switch (presence) {
      case PeerPresence.online:
        mark = Container(
            width: size,
            height: size,
            decoration: const BoxDecoration(
                color: UiColor.ready, shape: BoxShape.circle));
      case PeerPresence.offline:
        mark = Container(
            width: size,
            height: size,
            decoration: BoxDecoration(
                shape: BoxShape.circle,
                border: Border.all(color: UiColor.faint, width: 1)));
      case PeerPresence.unknown:
        mark = Container(width: size, height: 1, color: UiColor.faint);
    }
    return Tooltip(
        message: translate(switch (presence) {
          PeerPresence.online => 'Online',
          PeerPresence.offline => 'Offline',
          PeerPresence.unknown => 'Status unknown',
        }),
        child: SizedBox(
            width: size,
            height: UiSpace.rowActionHitSize,
            child: Center(child: mark)));
  }
}

/// One device row, shared by the grouped overview, the favourites and the
/// recent-connections list. The action column always reserves the star
/// slot so the icons line up across rows.
class DeviceRow extends StatelessWidget {
  final Peer peer;
  final bool local;
  final ValueChanged<Peer> onOpen;
  /// Unknown until the list has asked the server about this device.
  final PeerPresence presence;
  /// Whether the peer is in the favourites; null hides the star.
  final bool? favorite;
  final ValueChanged<Peer>? onToggleFavorite;
  /// Rows nested in a section card draw no border of their own.
  final bool bordered;
  const DeviceRow(
      {super.key,
      required this.peer,
      required this.local,
      required this.onOpen,
      this.presence = PeerPresence.unknown,
      this.favorite,
      this.onToggleFavorite,
      this.bordered = true});

  static bool mobile(Peer p) =>
      ['android', 'ios', 'ipados'].contains(p.platform.toLowerCase());

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final canFavorite = favorite != null && !local;
    return Material(
        color: Colors.white,
        shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(UiSpace.rowCardRadius),
            side: bordered
                ? const BorderSide(color: UiColor.border)
                : BorderSide.none),
        child: InkWell(
            onTap: local ? null : () => onOpen(peer),
            borderRadius: BorderRadius.circular(UiSpace.rowCardRadius),
            hoverColor: UiColor.surfaceHover,
            child: SizedBox(
                height: UiSpace.rowCardHeight,
                child: Padding(
                    padding: const EdgeInsets.symmetric(
                        horizontal: UiSpace.rowCardPaddingX),
                    child: Row(children: [
                      Container(
                          width: UiSpace.rowIconSize,
                          height: UiSpace.rowIconSize,
                          decoration: BoxDecoration(
                              color: UiColor.primary,
                              borderRadius:
                                  BorderRadius.circular(UiSpace.rowIconRadius)),
                          child: Icon(
                              mobile(peer)
                                  ? Icons.phone_android
                                  : Icons.desktop_windows_outlined,
                              color: Colors.white,
                              size: 18)),
                      const SizedBox(width: UiSpace.rowIconGap),
                      if (!local) ...[
                        PresenceDot(presence: presence),
                        const SizedBox(width: UiSpace.statusDotGap),
                      ],
                      // The name and its badge take the whole middle, so the
                      // action icons land on the card's right edge instead of
                      // drifting with the length of the name.
                      Expanded(
                          child: Row(children: [
                        Flexible(
                            child: Text(deviceName(peer),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: UiType.rowTitle)),
                        if (local) ...[
                          const SizedBox(width: UiSpace.rowBadgeGap),
                          Container(
                              height: UiSpace.tagHeight,
                              alignment: Alignment.center,
                              padding: const EdgeInsets.symmetric(
                                  horizontal: UiSpace.tagPaddingX),
                              decoration: BoxDecoration(
                                  color: UiColor.primaryTint,
                                  borderRadius:
                                      BorderRadius.circular(UiSpace.tagRadius)),
                              child: Text(zh ? '本机' : 'This device',
                                  style: UiType.tag)),
                        ],
                      ])),
                      SizedBox(
                          width: UiSpace.rowActionHitSize,
                          height: UiSpace.rowActionHitSize,
                          child: canFavorite
                              ? IconButton(
                                  padding: EdgeInsets.zero,
                                  iconSize: UiSpace.rowActionIconSize,
                                  tooltip: favorite!
                                      ? (zh ? '取消收藏' : 'Remove from favourites')
                                      : (zh ? '收藏' : 'Add to favourites'),
                                  icon: Icon(
                                      favorite! ? Icons.star : Icons.star_border,
                                      color: favorite!
                                          ? UiColor.favorite
                                          : UiColor.faint),
                                  onPressed: onToggleFavorite == null
                                      ? null
                                      : () => onToggleFavorite!(peer))
                              : null),
                      const SizedBox(width: UiSpace.rowActionGap),
                      SizedBox(
                          width: UiSpace.rowActionHitSize,
                          child: local
                              ? null
                              : const Icon(Icons.chevron_right,
                                  size: UiSpace.rowActionIconSize,
                                  color: UiColor.faint)),
                    ])))));
  }
}
