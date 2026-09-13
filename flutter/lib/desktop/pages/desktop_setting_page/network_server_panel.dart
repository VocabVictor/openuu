part of 'desktop_setting_page.dart';

/// The ID / relay server settings as an in-shell panel: four fields, a
/// clipboard import that only fills the fields, Reset and Save. Nothing is
/// written until Save succeeds.
class _ServerPanel extends StatefulWidget {
  final VoidCallback close;
  final VoidCallback onSaved;
  const _ServerPanel({required this.close, required this.onSaved});

  @override
  State<_ServerPanel> createState() => _ServerPanelState();
}

class _ServerPanelState extends State<_ServerPanel> {
  final _controllers = List.generate(4, (_) => TextEditingController());
  final _errors = [''.obs, ''.obs, ''.obs];
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    for (final c in _controllers) {
      c.dispose();
    }
    super.dispose();
  }

  Future<void> _load() async {
    Map<String, dynamic> options = {};
    try {
      options = jsonDecode(await bind.mainGetOptions());
    } catch (e) {
      debugPrint("Invalid server config: $e");
    }
    final config = ServerConfig.fromOptions(options);
    _controllers[0].text = config.idServer;
    _controllers[1].text = config.relayServer;
    _controllers[2].text = config.apiServer;
    _controllers[3].text = config.key;
    for (final e in _errors) {
      e.value = '';
    }
  }

  Future<void> _importClipboard() async {
    final text = (await Clipboard.getData(Clipboard.kTextPlain))?.text?.trim();
    if (text == null || text.isEmpty) return;
    try {
      final config = ServerConfig.decode(text);
      _controllers[0].text = config.idServer;
      _controllers[1].text = config.relayServer;
      _controllers[2].text = config.apiServer;
      _controllers[3].text = config.key;
    } catch (e) {
      showToast(translate('Invalid server configuration'));
    }
  }

  Future<void> _save() async {
    setState(() => _busy = true);
    final ok = await setServerConfig(
        _controllers,
        _errors,
        ServerConfig(
            idServer: _controllers[0].text,
            relayServer: _controllers[1].text,
            apiServer: _controllers[2].text,
            key: _controllers[3].text));
    if (!mounted) return;
    setState(() => _busy = false);
    if (ok) {
      showToast(translate('Successful'));
      widget.onSaved();
      widget.close();
    }
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final locked = isOptionFixed('custom-rendezvous-server');
    return Obx(() =>
        Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          SettingsPanelField(
              label: translate('ID Server'),
              controller: _controllers[0],
              error: _errors[0].value,
              hint: 'host:port',
              enabled: !locked && !_busy),
          SettingsPanelField(
              label: translate('Relay Server'),
              controller: _controllers[1],
              error: _errors[1].value,
              hint: 'host:port',
              enabled: !locked && !_busy),
          SettingsPanelField(
              label: translate('API Server'),
              controller: _controllers[2],
              error: _errors[2].value,
              hint: 'https://host:port',
              enabled: !locked && !_busy),
          SettingsPanelField(
              label: 'Key',
              controller: _controllers[3],
              monospace: true,
              lines: 3,
              enabled: !locked && !_busy),
          SettingsPanelFooter(buttons: [
            _secondaryButton(context, 'Import server config',
                locked || _busy ? null : _importClipboard),
            _secondaryButton(context, zh ? '重置' : 'Reset', _busy ? null : _load),
            _primaryButton(context, zh ? '保存' : 'Save', locked || _busy ? null : _save),
          ]),
        ]));
  }
}
