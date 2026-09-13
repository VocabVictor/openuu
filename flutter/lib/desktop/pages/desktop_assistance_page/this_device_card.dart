part of 'desktop_assistance_page.dart';

extension _ThisDeviceCard on _DesktopAssistancePageState {
  Widget _thisDeviceCard() {
    final temporary = widget.temporaryPassword;
    return _card(
        context,
        Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: UiSpace.s4,
            runSpacing: UiSpace.s2,
            children: [
              Row(mainAxisSize: MainAxisSize.min, children: [
                Text(translate('This device'),
                    style: UiType.of(context).sectionTitle),
                const SizedBox(width: UiSpace.rowMetaGap),
                ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 200),
                    child: Text(widget.deviceName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: UiType.of(context).caption)),
                const SizedBox(width: UiSpace.rowMetaGap),
                Container(
                    width: UiSpace.statusDotSize,
                    height: UiSpace.statusDotSize,
                    decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: widget.online
                            ? UiColor.of(context).ready
                            : UiColor.of(context).faint)),
                const SizedBox(width: UiSpace.statusDotGap),
                Text(widget.online ? translate('Online') : translate('Not ready'),
                    style: UiType.of(context).caption),
              ]),
              Row(mainAxisSize: MainAxisSize.min, children: [
                Text(translate('Allow remote assistance'),
                    style:
                        UiType.of(context).rowTitle.copyWith(fontWeight: FontWeight.w400)),
                const SizedBox(width: UiSpace.s2),
                SizedBox(
                    height: UiSpace.controlHeight,
                    child: FittedBox(
                        child: Switch(
                            value: widget.enabled,
                            activeColor: UiColor.of(context).primary,
                            onChanged: _busy
                                ? null
                                : (value) async {
                                    _setState(() => _busy = true);
                                    try {
                                      await widget.onEnable(value);
                                    } finally {
                                      if (mounted) {
                                        _setState(() => _busy = false);
                                      }
                                    }
                                  }))),
              ]),
            ]),
        Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          SizedBox(
              width: 200,
              child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                _caption(context, translate('This device ID')),
                const SizedBox(height: UiSpace.s2),
                SelectableText(widget.deviceId,
                    style: UiType.of(context).deviceId),
              ])),
          const SizedBox(width: UiSpace.s8),
          Expanded(child: _passwordColumn(temporary)),
          const SizedBox(width: UiSpace.s8),
          Padding(
              // Centre the button on the value row below the captions.
              padding: const EdgeInsets.only(top: 18 + UiSpace.s2),
              child: SizedBox(
                  height: UiSpace.controlHeight,
                  child: OutlinedButton(
                      style: OutlinedButton.styleFrom(
                          padding: const EdgeInsets.symmetric(
                              horizontal: UiSpace.buttonPaddingX),
                          shape: RoundedRectangleBorder(
                              borderRadius:
                                  BorderRadius.circular(UiSpace.buttonRadius)),
                          textStyle: UiType.of(context).button),
                      onPressed: !widget.enabled
                          ? null
                          : () async {
                              final passwordLine = temporary
                                  ? '\n${translate('Password')}: ${widget.password}'
                                  : '';
                              final text =
                                  'OpenUU\nID: ${widget.deviceId}$passwordLine';
                              await Clipboard.setData(ClipboardData(text: text));
                              if (mounted) {
                                _setState(() => _copied = true);
                              }
                            },
                      child: Text(_copied
                          ? translate('Copied')
                          : translate('Copy and share'))))),
          const SizedBox(width: UiSpace.s2),
          Padding(
              padding: const EdgeInsets.only(top: 18 + UiSpace.s2),
              child: SizedBox(
                  width: UiSpace.controlHeight,
                  height: UiSpace.controlHeight,
                  child: IconButton(
                      padding: EdgeInsets.zero,
                      iconSize: 18,
                      tooltip: translate('Share as QR code'),
                      onPressed: !widget.enabled
                          ? null
                          : () => _shareQrDialog(context, temporary),
                      icon: Icon(Icons.qr_code_2_outlined,
                          color: UiColor.of(context).textSecondary)))),
        ]));
  }

  /// An openuu://config QR code with the server settings and, when a
  /// one-time password is showing, a connect{id,password} entry so the
  /// phone that scans it lands straight on this device.
  Future<void> _shareQrDialog(BuildContext context,
      bool temporary) async {
    final reply = await bind.mainEncodeShareConfigWithConnect(
        optionKeys: [],
        id: widget.deviceId,
        password: temporary ? widget.password : '');
    Map<String, dynamic> json = {};
    try {
      json = jsonDecode(reply) as Map<String, dynamic>;
    } catch (_) {}
    if (json['ok'] != true) {
      showToast(json['error']?.toString() ?? translate('Cannot build the QR code'));
      return;
    }
    final payload = json['payload'].toString();
    gFFI.dialogManager.show((setState, close, context) => CustomAlertDialog(
        titlePadding: EdgeInsets.zero,
        contentBoxConstraints: const BoxConstraints(minWidth: 352, maxWidth: 352),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          SizedBox(
              height: 48,
              child: Row(children: [
                Expanded(
                    child: Text(translate('Share as QR code'),
                        style: UiType.of(context)
                            .sectionTitle
                            .copyWith(fontSize: 16))),
                IconButton(
                    iconSize: 16,
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
                    icon: Icon(Icons.close, color: UiColor.of(context).muted),
                    onPressed: close),
              ])),
          const SizedBox(height: UiSpace.s2),
          Container(
              color: UiColor.of(context).surface,
              padding: const EdgeInsets.all(UiSpace.s2),
              child: QrImageView(data: payload, version: QrVersions.auto, size: 200, gapless: false)),
          const SizedBox(height: UiSpace.s3),
          Text(
              temporary
                  ? translate('Scanning with OpenUU on a phone connects to this device; the code carries the current one-time password, do not forward it.')
                  : translate('Scanning with OpenUU on a phone connects to this device.'),
              style: UiType.of(context).caption,
              textAlign: TextAlign.center),
          const SizedBox(height: UiSpace.s6),
        ]),
        onCancel: close));
  }

  Widget _passwordColumn(bool temporary) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        _verificationMenu(),
        const SizedBox(height: UiSpace.s2),
        Row(children: [
          Flexible(
              child: Text(
                  temporary
                      ? (_visible ? widget.password : '••••••••')
                      : translate('Authentication configured'),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: temporary
                      ? UiType.of(context)
                          .deviceId
                          .copyWith(fontSize: 22, height: 1.3)
                      : UiType.of(context).rowTitle)),
          const SizedBox(width: UiSpace.rowIconGap),
          if (temporary)
            _actionIcon(
                _visible ? Icons.visibility_outlined : Icons.visibility_off_outlined,
                translate('Show / hide password'),
                () => _setState(() => _visible = !_visible)),
          if (temporary)
            _actionIcon(Icons.refresh, translate('Refresh password'),
                widget.onRefresh),
          _actionIcon(Icons.settings_outlined,
              translate('Authentication settings'), widget.onSecurity),
        ]),
        const SizedBox(height: 6),
        _caption(context, temporary
            ? translate('One-time password, renewed after each session')
            : translate('Uses your security settings')),
      ]);

  Widget _actionIcon(IconData icon, String tooltip, VoidCallback onTap) =>
      Padding(
          padding: const EdgeInsets.only(right: UiSpace.rowActionGap),
          child: SizedBox(
              width: 28,
              height: 28,
              child: IconButton(
                  padding: EdgeInsets.zero,
                  iconSize: UiSpace.rowActionIconSize,
                  tooltip: tooltip,
                  onPressed: onTap,
                  icon: Icon(icon, color: UiColor.of(context).textSecondary))));

  /// The verification method as a menu anchored under its trigger: same
  /// left edge, at least the trigger's width, current value ticked.
  Widget _verificationMenu() =>
      LayoutBuilder(builder: (context, bounds) {
        final entries = [
          (_useTemporaryPassword, translate('One-time password only')),
          (_usePermanentPassword, translate('Permanent password only')),
          (_useBothPasswords, translate('Use both passwords')),
        ];
        return PopupMenuButton<String>(
            tooltip: translate('Change verification'),
            position: PopupMenuPosition.under,
            offset: const Offset(0, UiSpace.menuOffset),
            constraints: BoxConstraints(minWidth: bounds.maxWidth),
            padding: EdgeInsets.zero,
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(UiSpace.menuRadius),
                side: BorderSide(color: UiColor.of(context).border)),
            elevation: 4,
            color: UiColor.of(context).surface,
            onSelected: widget.onVerificationChanged,
            itemBuilder: (_) => [
                  for (final e in entries)
                    PopupMenuItem<String>(
                        value: e.$1,
                        height: UiSpace.menuItemHeight,
                        padding: const EdgeInsets.symmetric(
                            horizontal: UiSpace.menuItemPaddingX),
                        child: Row(children: [
                          Expanded(
                              child: Text(e.$2,
                                  style: UiType.of(context).rowTitle
                                      .copyWith(fontWeight: FontWeight.w400))),
                          if (e.$1 == widget.verificationMethod)
                            Icon(Icons.check,
                                size: 14, color: UiColor.of(context).primary),
                        ])),
                ],
            child: SizedBox(
                height: 18,
                child: Row(children: [
                  Flexible(child: _caption(context, widget.verification)),
                  const SizedBox(width: UiSpace.s1),
                  Icon(Icons.expand_more,
                      size: 14, color: UiColor.of(context).muted)
                ])));
      });
}
