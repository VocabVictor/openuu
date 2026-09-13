part of 'desktop_setting_page.dart';

/// The Socks5 / HTTP(S) proxy as an in-shell panel: server, username and
/// password fields, Reset and Save. The server is validated before anything
/// is written; leaving it empty removes the proxy.
class _ProxyPanel extends StatefulWidget {
  final VoidCallback close;
  final VoidCallback onSaved;
  const _ProxyPanel({required this.close, required this.onSaved});

  @override
  State<_ProxyPanel> createState() => _ProxyPanelState();
}

class _ProxyPanelState extends State<_ProxyPanel> {
  final _proxy = TextEditingController();
  final _user = TextEditingController();
  final _password = TextEditingController();
  String _error = '';
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _proxy.dispose();
    _user.dispose();
    _password.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    final socks = await bind.mainGetSocks();
    if (!mounted) return;
    setState(() {
      _proxy.text = socks.length == 3 ? socks[0] : '';
      _user.text = socks.length == 3 ? socks[1] : '';
      _password.text = socks.length == 3 ? socks[2] : '';
      _error = '';
    });
  }

  Future<void> _save() async {
    setState(() {
      _busy = true;
      _error = '';
    });
    final proxy = _proxy.text.trim();
    if (proxy.isNotEmpty) {
      var domainPort = proxy;
      if (domainPort.contains('://')) {
        domainPort = domainPort.split('://')[1];
      }
      final message = translate(await bind.mainTestIfValidServer(
          server: domainPort, testWithProxy: false));
      if (!mounted) return;
      if (message.isNotEmpty) {
        setState(() {
          _busy = false;
          _error = message;
        });
        return;
      }
    }
    await bind.mainSetSocks(
        proxy: proxy,
        username: _user.text.trim(),
        password: _password.text.trim());
    if (!mounted) return;
    setState(() => _busy = false);
    showToast(translate('Successful'));
    widget.onSaved();
    widget.close();
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final locked = isOptionFixed('proxy-url');
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      SettingsPanelField(
          label: translate('Server'),
          controller: _proxy,
          error: _error,
          hint: translate('default_proxy_tip'),
          enabled: !locked && !_busy),
      SettingsPanelField(
          label: translate('Username'),
          controller: _user,
          enabled: !locked && !_busy),
      SettingsPanelField(
          label: translate('Password'),
          controller: _password,
          obscure: true,
          enabled: !locked && !_busy),
      SettingsPanelFooter(buttons: [
        _secondaryButton(zh ? '重置' : 'Reset', _busy ? null : _load),
        _primaryButton(zh ? '保存' : 'Save', locked || _busy ? null : _save),
      ]),
    ]);
  }
}
