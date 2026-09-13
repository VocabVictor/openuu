part of 'desktop_assistance_page.dart';

extension _PartnerCard on _DesktopAssistancePageState {
  Widget _partnerCard(String Function(String, String) t) {
    final fieldStyle = OutlineInputBorder(
        borderRadius: BorderRadius.circular(5),
        borderSide: const BorderSide(color: Color(0xffdce2e7)));
    return _card(
        Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(t('远控伙伴设备', 'Connect to a partner'),
              style:
                  const TextStyle(fontSize: 18, fontWeight: FontWeight.w600)),
          const SizedBox(height: 6),
          _caption(t('通过设备 ID 连接，并按对方设置完成验证',
              'Connect using a device ID, then authenticate with your partner.')),
        ]),
        Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          _caption(t('伙伴的设备 ID', 'Partner device ID')),
          const SizedBox(height: 14),
          Wrap(
              spacing: 14,
              runSpacing: 12,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                SizedBox(
                    width: 240,
                    child: TextField(
                        controller: _remoteId,
                        onChanged: (_) => _setState(() {}),
                        onSubmitted: (id) {
                          if (id.trim().isNotEmpty) {
                            widget.onConnect(id.trim());
                          }
                        },
                        decoration: InputDecoration(
                            hintText: t('请输入设备 ID', 'Enter device ID'),
                            filled: true,
                            fillColor: Colors.white,
                            isDense: true,
                            contentPadding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 13),
                            border: fieldStyle,
                            enabledBorder: fieldStyle))),
                SizedBox(
                    width: 140,
                    height: 42,
                    child: ElevatedButton(
                        style: ElevatedButton.styleFrom(
                            backgroundColor: DesktopWelcomePage.blue,
                            foregroundColor: Colors.white,
                            elevation: 0,
                            shape: RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(5))),
                        onPressed: _remoteId.text.trim().isEmpty
                            ? null
                            : () => widget.onConnect(_remoteId.text.trim()),
                        child: Text(t('连接', 'Connect')))),
              ]),
        ]));
  }
}
