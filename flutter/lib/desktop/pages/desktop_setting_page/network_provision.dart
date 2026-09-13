part of 'desktop_setting_page.dart';

/// Configuration provisioning on the desktop network tab
/// (docs/server-config-provisioning.md §4): a share row that shows the
/// current server settings as an `openuu://config/…` QR code, and an import
/// row (clipboard or file) that previews the decoded server values in a
/// confirmation dialog before anything is applied.
extension _NetworkProvision on _NetworkState {
  List<Widget> _provisionRows(BuildContext context, bool zh) => [
        SettingsRow(
            label: zh ? '分享配置二维码' : 'Share configuration QR code',
            enabled: !locked,
            onTap: () => _shareConfigDialog(context, zh),
            control: const Icon(Icons.qr_code_2_outlined,
                size: 16, color: UiColor.muted)),
        _settingRow(
            context,
            zh ? '导入配置' : 'Import configuration',
            Row(mainAxisSize: MainAxisSize.min, children: [
              _secondaryButton(zh ? '从剪贴板' : 'From clipboard',
                  locked ? null : () => _importFromClipboard(context, zh)),
              const SizedBox(width: UiSpace.s2),
              _secondaryButton(zh ? '从文件' : 'From file',
                  locked ? null : () => _importFromFile(context, zh)),
            ]),
            description: zh
                ? '支持 openuu://config 链接与 JSON 配置文件，导入前会先确认'
                : 'Accepts openuu://config links and JSON files; asks before applying'),
      ];

  Future<void> _shareConfigDialog(BuildContext context, bool zh) async {
    final reply = _json(await bind.mainEncodeShareConfig(optionKeys: []));
    if (reply['ok'] != true) {
      showToast(reply['error']?.toString() ??
          (zh ? '无法生成二维码' : 'Cannot build the QR code'));
      return;
    }
    final payload = reply['payload'].toString();
    gFFI.dialogManager.show((setState, close, context) => CustomAlertDialog(
        titlePadding: EdgeInsets.zero,
        contentBoxConstraints: const BoxConstraints(
            minWidth: UiSpace.dialogContentWidth,
            maxWidth: UiSpace.dialogContentWidth),
        content: Column(mainAxisSize: MainAxisSize.min, children: [
          _dialogTitle(zh ? '分享配置二维码' : 'Share configuration', close),
          const SizedBox(height: UiSpace.s2),
          Container(
              color: Colors.white,
              padding: const EdgeInsets.all(UiSpace.s2),
              child: QrImageView(
                  data: payload,
                  version: QrVersions.auto,
                  size: 200,
                  gapless: false)),
          const SizedBox(height: UiSpace.s3),
          Text(
              zh
                  ? '用手机 OpenUU 扫码即可导入服务器设置；二维码不含密码等秘密。'
                  : 'Scan with OpenUU on a phone to import the server settings; the code carries no secrets.',
              style: UiType.caption,
              textAlign: TextAlign.center),
          const SizedBox(height: UiSpace.s6),
          Row(mainAxisAlignment: MainAxisAlignment.end, children: [
            _secondaryButton(zh ? '复制链接' : 'Copy link', () async {
              await Clipboard.setData(ClipboardData(text: payload));
              showToast(zh ? '已复制' : 'Copied');
            }, height: UiSpace.controlHeight),
            const SizedBox(width: UiSpace.s2),
            _primaryButton(zh ? '关闭' : 'Close', close,
                height: UiSpace.controlHeight),
          ]),
        ]),
        onCancel: close));
  }

  Future<void> _importFromClipboard(BuildContext context, bool zh) async {
    final text = (await Clipboard.getData(Clipboard.kTextPlain))?.text?.trim();
    if (text == null || text.isEmpty) {
      showToast(zh ? '剪贴板为空' : 'The clipboard is empty');
      return;
    }
    await _confirmImport(context, zh, text, trusted: false);
  }

  Future<void> _importFromFile(BuildContext context, bool zh) async {
    final picked = await FilePicker.platform.pickFiles(
        type: FileType.custom, allowedExtensions: ['json', 'toml', 'txt']);
    final path = picked?.files.single.path;
    if (path == null) return;
    final text = await File(path).readAsString();
    await _confirmImport(context, zh, text, trusted: true);
  }

  /// Shows the decoded server values and applies the text only on confirm.
  Future<void> _confirmImport(BuildContext context, bool zh, String text,
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
              width: 96, child: Text(label, style: UiType.caption)),
          Expanded(
              child: Text(value.isEmpty ? '—' : value,
                  style: UiType.rowTitle.copyWith(
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
              _dialogTitle(zh ? '导入配置' : 'Import configuration', close),
              const SizedBox(height: UiSpace.s2),
              line(translate('ID Server'), '${server['id'] ?? ''}'),
              line(translate('Relay Server'), '${server['relay'] ?? ''}'),
              line(translate('API Server'), '${server['api'] ?? ''}'),
              line('Key', (server['key'] ?? '').toString().isEmpty
                  ? ''
                  : (zh ? '已包含' : 'included')),
              if (locked.isNotEmpty)
                Padding(
                    padding: const EdgeInsets.only(top: UiSpace.s2),
                    child: Text(
                        zh
                            ? '以下选项将被锁定：${locked.join(', ')}'
                            : 'These options will be locked: ${locked.join(', ')}',
                        style: UiType.caption)),
              const SizedBox(height: UiSpace.s6),
              Row(mainAxisAlignment: MainAxisAlignment.end, children: [
                _secondaryButton('Cancel', close,
                    height: UiSpace.controlHeight),
                const SizedBox(width: UiSpace.s2),
                _primaryButton(zh ? '导入' : 'Import', () async {
                  final report = _json(await bind.mainImportConfigText(
                      text: text, trusted: trusted));
                  close();
                  if (report['ok'] == true) {
                    final applied =
                        ((report['report'] as Map?)?['applied'] as List?)
                                ?.length ??
                            0;
                    showToast(zh
                        ? '已导入 $applied 项设置'
                        : 'Imported $applied settings');
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
Widget _dialogTitle(String title, VoidCallback close) => SizedBox(
    height: UiSpace.dialogTitleHeight,
    child: Row(children: [
      Expanded(
          child: Text(title, style: UiType.sectionTitle.copyWith(fontSize: 16))),
      IconButton(
          iconSize: 16,
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
          icon: const Icon(Icons.close, color: UiColor.muted),
          onPressed: close),
    ]));
