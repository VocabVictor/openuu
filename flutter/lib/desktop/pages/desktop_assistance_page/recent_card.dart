part of 'desktop_assistance_page.dart';

extension _RecentCard on _DesktopAssistancePageState {
  Widget _recentCard(String Function(String, String) t) => _card(
      Padding(
          padding: const EdgeInsets.symmetric(vertical: UiSpace.s3),
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(t('最近连接', 'Recent connections'), style: UiType.sectionTitle),
            const SizedBox(height: UiSpace.s1),
            _caption(t('点击设备可再次连接', 'Tap a device to connect again')),
          ])),
      widget.recentPeers.isEmpty
          ? EmptyRow(text: t('暂无最近连接', 'No recent connections'))
          : Column(children: [
              for (var i = 0; i < widget.recentPeers.length; i++) ...[
                if (i > 0) const Divider(height: 1, color: UiColor.border),
                DeviceRow(
                    peer: widget.recentPeers[i],
                    local: false,
                    bordered: false,
                    onOpen: widget.onOpenRecent),
              ],
            ]),
      divider: false);
}
