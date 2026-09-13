part of 'desktop_assistance_page.dart';

extension _RecentCard on _DesktopAssistancePageState {
  Widget _recentCard(String Function(String, String) t) => _card(
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text(t('最近连接', 'Recent connections'),
            style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w600)),
        const SizedBox(height: 6),
        _caption(t('点击设备卡片可再次连接', 'Tap a device to connect again')),
      ]),
      widget.recentPeers.isEmpty
          ? Padding(
              padding: const EdgeInsets.symmetric(vertical: 8),
              child: _caption(t('暂无最近连接', 'No recent connections')))
          : Column(children: [
              for (final peer in widget.recentPeers)
                DeviceCard(
                    peer: peer, local: false, onOpen: widget.onOpenRecent),
            ]));
}
