part of 'desktop_assistance_page.dart';

extension _PartnerCard on _DesktopAssistancePageState {
  Widget _partnerCard() {
    final fieldStyle = OutlineInputBorder(
        borderRadius: BorderRadius.circular(UiSpace.inputRadius),
        borderSide: const BorderSide(color: UiColor.border));
    return _card(
        Padding(
            padding: const EdgeInsets.symmetric(vertical: UiSpace.s3),
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(translate('Connect to a partner'),
                  style: UiType.sectionTitle),
              const SizedBox(height: UiSpace.s1),
              _caption(translate('Connect using a device ID, then authenticate with your partner.')),
            ])),
        Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          _caption(translate('Partner device ID')),
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
                        hintText: translate('Enter device ID'),
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
                    child: Text(translate('Connect')))),
          ]),
        ]),
        divider: false);
  }
}
