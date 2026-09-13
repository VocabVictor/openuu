part of 'desktop_setting_page.dart';

extension _NetworkDesktop on _NetworkState {
  /// The network tab on the desktop shell: a Server group of navigation
  /// rows with value summaries, then a Connection group of switch rows
  /// whose former tooltips are resident subtitles.
  Widget _networkDesktop(BuildContext context,
      {required bool hideServer,
      required bool hideProxy,
      required bool hideWebSocket}) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';

    Future<String> serverSummary() async {
      Map<String, dynamic> options = {};
      try {
        options = jsonDecode(await bind.mainGetOptions());
      } catch (e) {
        debugPrint("Invalid server config: $e");
      }
      final id = ServerConfig.fromOptions(options).idServer;
      return id.isEmpty ? (zh ? '使用默认服务器' : 'Default server') : id;
    }

    Future<String> proxySummary() async {
      final socks = await bind.mainGetSocks();
      return socks.length == 3 && socks[0].isNotEmpty
          ? socks[0]
          : (zh ? '未使用代理' : 'No proxy');
    }

    Widget boolRow(String label, String tip, String key) => _switchRow(
        context, label, mainGetBoolOptionSync(key), (value) {
      mainSetBoolOption(key, value);
      _setState(() {});
    }, enabled: !locked && !isOptionFixed(key), description: translate(tip));

    final outgoingOnly = bind.isOutgoingOnly();
    return Column(children: [
      if (!hideServer || !hideProxy)
        _group(zh ? '服务器' : 'Server', [
          if (!hideServer)
            futureBuilder(
                future: serverSummary(),
                hasData: (summary) => SettingsExpandPanel(
                    label: translate('ID/Relay Server'),
                    summary: summary.toString(),
                    enabled: !locked,
                    panel: (context, close) => _ServerPanel(
                        close: close, onSaved: () => _setState(() {})))),
          if (!hideProxy)
            futureBuilder(
                future: proxySummary(),
                hasData: (summary) => SettingsExpandPanel(
                    label: translate('Socks5/Http(s) Proxy'),
                    summary: summary.toString(),
                    enabled: !locked,
                    panel: (context, close) => _ProxyPanel(
                        close: close, onSaved: () => _setState(() {})))),
        ]),
      futureBuilder(
          future: bind.mainIsUsingPublicServer(),
          hasData: (isUsingPublicServer) {
            final rows = [
              if (!hideWebSocket)
                boolRow('Use WebSocket', 'websocket_tip',
                    kOptionAllowWebSocket),
              if (isUsingPublicServer != true && !outgoingOnly)
                _switchRow(
                    context,
                    'Disable UDP',
                    bind.mainGetOptionSync(key: kOptionDisableUdp) == 'Y',
                    (value) async {
                      await bind.mainSetOption(
                          key: kOptionDisableUdp, value: value ? 'Y' : 'N');
                      _setState(() {});
                    },
                    enabled: !locked && !isOptionFixed(kOptionDisableUdp),
                    description: translate('disable-udp-tip')),
              if (isUsingPublicServer != true)
                boolRow(
                    'Allow insecure TLS fallback',
                    'allow-insecure-tls-fallback-tip',
                    kOptionAllowInsecureTLSFallback),
            ];
            if (rows.isEmpty) return const Offstage();
            return _group(zh ? '连接方式' : 'Connection', rows);
          }),
    ]);
  }
}
