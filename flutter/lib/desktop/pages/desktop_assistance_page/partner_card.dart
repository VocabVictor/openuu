part of 'desktop_assistance_page.dart';

extension _PartnerCard on _DesktopAssistancePageState {
  Widget _partnerCard(String Function(String, String) t) {
    final fieldStyle = OutlineInputBorder(
        borderRadius: BorderRadius.circular(UiSpace.inputRadius),
        borderSide: const BorderSide(color: UiColor.border));
    return _card(
        Padding(
            padding: const EdgeInsets.symmetric(vertical: UiSpace.s3),
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(t('远控伙伴设备', 'Connect to a partner'),
                  style: UiType.sectionTitle),
              const SizedBox(height: UiSpace.s1),
              _caption(t('通过设备 ID 连接，并按对方设置完成验证',
                  'Connect using a device ID, then authenticate with your partner.')),
            ])),
        Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          _caption(t('伙伴的设备 ID', 'Partner device ID')),
          const SizedBox(height: UiSpace.fieldLabelGap),
          Row(children: [
            SizedBox(
                width: 240,
                height: UiSpace.controlHeight,
                child: TextField(
                    controller: _remoteId,
                    onChanged: (_) => _setState(() {}),
                    onSubmitted: (id) {
                      if (id.trim().isNotEmpty) {
                        widget.onConnect(id.trim());
                      }
                    },
                    style: UiType.rowTitle.copyWith(fontWeight: FontWeight.w400),
                    decoration: InputDecoration(
                        hintText: t('请输入设备 ID', 'Enter device ID'),
                        hintStyle: UiType.caption.copyWith(fontSize: 13),
                        filled: true,
                        fillColor: Colors.white,
                        isDense: true,
                        contentPadding: const EdgeInsets.symmetric(
                            horizontal: UiSpace.inputPaddingX, vertical: 8),
                        border: fieldStyle,
                        enabledBorder: fieldStyle))),
            const SizedBox(width: UiSpace.fieldGap),
            SizedBox(
                height: UiSpace.controlHeight,
                child: ElevatedButton(
                    style: ElevatedButton.styleFrom(
                        backgroundColor: UiColor.primary,
                        foregroundColor: Colors.white,
                        elevation: 0,
                        minimumSize: const Size(72, UiSpace.controlHeight),
                        padding: const EdgeInsets.symmetric(
                            horizontal: UiSpace.buttonPaddingX),
                        textStyle: UiType.button,
                        shape: RoundedRectangleBorder(
                            borderRadius:
                                BorderRadius.circular(UiSpace.buttonRadius))),
                    onPressed: _remoteId.text.trim().isEmpty
                        ? null
                        : () => widget.onConnect(_remoteId.text.trim()),
                    child: Text(t('连接', 'Connect')))),
          ]),
        ]),
        divider: false);
  }
}
