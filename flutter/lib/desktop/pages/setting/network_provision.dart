part of 'setting.dart';

/// Configuration provisioning on the desktop network tab
/// (docs/server-config-provisioning.md §4): a share row that shows the
/// current server settings as an `openuu://config/…` QR code, and an import
/// row (clipboard or file) that previews the decoded server values in a
/// confirmation dialog before anything is applied.
extension _NetworkProvision on _NetworkState {
  List<Widget> _provisionRows(BuildContext context) => [
        SettingsRow(
            label: translate('Share configuration QR code'),
            enabled: !locked,
            onTap: () => _shareConfigDialog(context),
            control: Icon(Icons.qr_code_2_outlined,
                size: 16, color: UiColor.of(context).muted)),
        _settingRow(
            context,
            translate('Import configuration'),
            Row(mainAxisSize: MainAxisSize.min, children: [
              _secondaryButton(context, translate('From clipboard'),
                  locked ? null : () => _importFromClipboard(context)),
              const SizedBox(width: UiSpace.s2),
              _secondaryButton(context, translate('From file'),
                  locked ? null : () => _importFromFile(context)),
            ]),
            description: translate('Accepts openuu://config links and JSON files; asks before applying')),
      ];

  Future<void> _shareConfigDialog(BuildContext context) async {
    final reply = _json(await bind.mainEncodeShareConfig(optionKeys: []));
    if (reply['ok'] != true) {
      showToast(reply['error']?.toString() ??
          (translate('Cannot build the QR code')));
      return;
    }
    final payload = reply['payload'].toString();
    gFFI.dialogManager.show((setState, close, context) => CustomAlertDialog(
        titlePadding: EdgeInsets.zero,
        contentBoxConstraints: const BoxConstraints(
            minWidth: UiSpace.dialogContentWidth,
            maxWidth: UiSpace.dialogContentWidth),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          _dialogTitle(context, translate('Share configuration'), close),
          const SizedBox(height: UiSpace.s2),
          Container(
              // Not a themed surface: a QR code is read as dark modules on a
              // light quiet zone, and a scanner does not know about themes.
              color: Colors.white,
              padding: const EdgeInsets.all(UiSpace.s2),
              child: QrImageView(
                  data: payload,
                  version: QrVersions.auto,
                  size: 200,
                  gapless: false)),
          const SizedBox(height: UiSpace.s3),
          Text(
              translate('Scan with OpenUU on a phone to import the server settings; the code carries no secrets.'),
              style: UiType.of(context).caption,
              textAlign: TextAlign.center),
          const SizedBox(height: UiSpace.s6),
          Row(mainAxisAlignment: MainAxisAlignment.end, children: [
            _secondaryButton(context, translate('Copy link'), () async {
              await Clipboard.setData(ClipboardData(text: payload));
              showToast(translate('Copied'));
            }, height: UiSpace.controlHeight),
            const SizedBox(width: UiSpace.s2),
            _primaryButton(context, translate('Close'), close,
                height: UiSpace.controlHeight),
          ]),
        ]),
        onCancel: close));
  }

  Future<void> _importFromClipboard(BuildContext context) async {
    final text = (await Clipboard.getData(Clipboard.kTextPlain))?.text?.trim();
    if (text == null || text.isEmpty) {
      showToast(translate('The clipboard is empty'));
      return;
    }
    await _confirmImport(context, text, trusted: false);
  }

  Future<void> _importFromFile(BuildContext context) async {
    final picked = await FilePicker.platform.pickFiles(
        type: FileType.custom, allowedExtensions: ['json', 'toml', 'txt']);
    final path = picked?.files.single.path;
    if (path == null) return;
    final text = await File(path).readAsString();
    await _confirmImport(context, text, trusted: true);
  }

  /// Shows the decoded server values and applies the text only on confirm.
  Future<void> _confirmImport(BuildContext context, String text,
      {required bool trusted}) async {
    final preview = _json(await bind.mainPreviewConfigText(text: text));
    if (preview['ok'] != true) {
      showToast(preview['error']?.toString() ??
          translate('Invalid server configuration'));
      return;
    }
    final server = (preview['server'] as Map?) ?? {};
    final locked = (preview['locked'] as List?) ?? [];
    Widget line(String label, String value) => Padding(
        padding: const EdgeInsets.only(bottom: UiSpace.s1),
        child: Row(children: [
          SizedBox(
              width: 96, child: Text(label, style: UiType.of(context).caption)),
          Expanded(
              child: Text(value.isEmpty ? '—' : value,
                  style: UiType.of(context).rowTitle.copyWith(
                      fontSize: 13, fontWeight: FontWeight.w400))),
        ]));
    gFFI.dialogManager.show((setState, close, context) => CustomAlertDialog(
        titlePadding: EdgeInsets.zero,
        contentBoxConstraints: const BoxConstraints(
            minWidth: UiSpace.dialogContentWidth,
            maxWidth: UiSpace.dialogContentWidth),
        content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _dialogTitle(context, translate('Import configuration'), close),
              const SizedBox(height: UiSpace.s2),
              line(translate('ID Server'), '${server['id'] ?? ''}'),
              line(translate('Relay Server'), '${server['relay'] ?? ''}'),
              line(translate('API Server'), '${server['api'] ?? ''}'),
              line('Key', (server['key'] ?? '').toString().isEmpty
                  ? ''
                  : (translate('included'))),
              if (locked.isNotEmpty)
                Padding(
                    padding: const EdgeInsets.only(top: UiSpace.s2),
                    child: Text(
                        translate(
                            'These options will be locked: {${locked.join(', ')}}'),
                        style: UiType.of(context).caption)),
              const SizedBox(height: UiSpace.s6),
              Row(mainAxisAlignment: MainAxisAlignment.end, children: [
                _secondaryButton(context, 'Cancel', close,
                    height: UiSpace.controlHeight),
                const SizedBox(width: UiSpace.s2),
                _primaryButton(context, translate('Import'), () async {
                  final report = _json(await bind.mainImportConfigText(
                      text: text, trusted: trusted));
                  close();
                  if (report['ok'] == true) {
                    final applied =
                        ((report['report'] as Map?)?['applied'] as List?)
                                ?.length ??
                            0;
                    showToast(translate('Imported {$applied} settings'));
                    _setState(() {});
                  } else {
                    showToast(report['error']?.toString() ??
                        translate('Failed'));
                  }
                }, height: UiSpace.controlHeight),
              ]),
            ]),
        onCancel: close));
  }

  Map<String, dynamic> _json(String text) {
    try {
      final value = jsonDecode(text);
      if (value is Map<String, dynamic>) return value;
    } catch (_) {}
    return {'ok': false};
  }
}

/// A 48-high, left-aligned dialog title with a close icon.
Widget _dialogTitle(BuildContext context, String title, VoidCallback close) => SizedBox(
    height: UiSpace.dialogTitleHeight,
    child: Row(children: [
      Expanded(
          child: Text(title, style: UiType.of(context).sectionTitle.copyWith(fontSize: 16))),
      IconButton(
          iconSize: 16,
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
          icon: Icon(Icons.close, color: UiColor.of(context).muted),
          onPressed: close),
    ]));
