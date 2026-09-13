part of 'desktop_assistance_page.dart';

extension _RecentCard on _DesktopAssistancePageState {
  Widget _recentCard() => _card(
      context,
      Padding(
          padding: const EdgeInsets.symmetric(vertical: UiSpace.s3),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(translate('Recent connections'),
                style: UiType.of(context).sectionTitle),
            const SizedBox(height: UiSpace.s1),
            _caption(context, translate('Tap a device to connect again')),
          ])),
      widget.recentPeers.isEmpty
          ? EmptyRow(text: translate('No recent connections'))
          : Column(children: [
              for (var i = 0; i < widget.recentPeers.length; i++) ...[
                if (i > 0)
                  Divider(height: 1, color: UiColor.of(context).border),
                DeviceRow(
                    peer: widget.recentPeers[i],
                    local: false,
                    bordered: false,
                    onOpen: widget.onOpenRecent),
              ],
            ]),
      divider: false);
}
