import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'desktop_welcome_page.dart';

class DesktopAssistancePage extends StatefulWidget {
  final String deviceId, password, verification;
  final bool enabled, temporaryPassword;
  final Future<void> Function(bool) onEnable;
  final ValueChanged<String> onConnect;
  final VoidCallback onRefresh,
      onSecurity,
      onDevices,
      onFavorites,
      onSettings,
      onAccount;
  const DesktopAssistancePage(
      {super.key,
      required this.deviceId,
      required this.password,
      required this.verification,
      required this.enabled,
      required this.temporaryPassword,
      required this.onEnable,
      required this.onConnect,
      required this.onRefresh,
      required this.onSecurity,
      required this.onDevices,
      required this.onFavorites,
      required this.onSettings,
      required this.onAccount});
  @override
  State<DesktopAssistancePage> createState() => _DesktopAssistancePageState();
}

class _DesktopAssistancePageState extends State<DesktopAssistancePage> {
  final _remoteId = TextEditingController();
  bool _visible = false, _busy = false, _copied = false;
  @override
  void dispose() {
    _remoteId.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    const muted = Color(0xff7b8492);
    final temporary = widget.temporaryPassword;
    Widget caption(String text) =>
        Text(text, style: const TextStyle(fontSize: 13, color: muted));
    Widget card(Widget heading, Widget content) => Container(
        decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: const Color(0xffdce2e7))),
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Padding(padding: const EdgeInsets.all(18), child: heading),
          const Divider(height: 1, color: Color(0xffe3e7eb)),
          Padding(padding: const EdgeInsets.all(18), child: content),
        ]));
    final fieldStyle = OutlineInputBorder(
        borderRadius: BorderRadius.circular(5),
        borderSide: const BorderSide(color: Color(0xffdce2e7)));
    return DesktopWelcomePage(
        assistanceSelected: true,
        onLogin: widget.onAccount,
        onDevices: widget.onDevices,
        onAssistance: () {},
        onFavorites: widget.onFavorites,
        onSettings: widget.onSettings,
        header: Text(t('远程协助', 'Remote assistance'),
            style: const TextStyle(fontSize: 28, fontWeight: FontWeight.w600)),
        content: LayoutBuilder(builder: (context, bounds) {
          final inset = (bounds.maxWidth * .055).clamp(20.0, 48.0);
          return SingleChildScrollView(
              padding: EdgeInsets.fromLTRB(inset, 20, inset, 32),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    card(
                        Wrap(
                            alignment: WrapAlignment.spaceBetween,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            spacing: 20,
                            runSpacing: 8,
                            children: [
                              Text(t('本设备', 'This device'),
                                  style: const TextStyle(
                                      fontSize: 18,
                                      fontWeight: FontWeight.w600)),
                              Row(mainAxisSize: MainAxisSize.min, children: [
                                Text(t('允许他人远程协助', 'Allow remote assistance'),
                                    style: const TextStyle(fontSize: 14)),
                                const SizedBox(width: 10),
                                Switch(
                                    value: widget.enabled,
                                    activeColor: DesktopWelcomePage.blue,
                                    onChanged: _busy
                                        ? null
                                        : (value) async {
                                            setState(() => _busy = true);
                                            try {
                                              await widget.onEnable(value);
                                            } finally {
                                              if (mounted) {
                                                setState(() => _busy = false);
                                              }
                                            }
                                          }),
                              ]),
                            ]),
                        Wrap(
                            spacing: 32,
                            runSpacing: 22,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: [
                              SizedBox(
                                  width: 200,
                                  child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        caption(t('本设备 ID', 'This device ID')),
                                        const SizedBox(height: 16),
                                        SelectableText(widget.deviceId,
                                            style: const TextStyle(
                                                fontSize: 27,
                                                fontWeight: FontWeight.w600,
                                                letterSpacing: .8)),
                                      ])),
                              SizedBox(
                                  width: 270,
                                  child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        InkWell(
                                            onTap: widget.onSecurity,
                                            child: Row(children: [
                                              Expanded(
                                                  child: caption(
                                                      widget.verification)),
                                              const Icon(Icons.expand_more,
                                                  size: 17, color: muted)
                                            ])),
                                        const SizedBox(height: 8),
                                        Row(children: [
                                          Expanded(
                                              child: Text(
                                                  temporary
                                                      ? (_visible
                                                          ? widget.password
                                                          : '••••••••')
                                                      : t('已配置验证方式',
                                                          'Authentication configured'),
                                                  maxLines: 1,
                                                  overflow:
                                                      TextOverflow.ellipsis,
                                                  style: TextStyle(
                                                      fontSize:
                                                          temporary ? 22 : 14,
                                                      letterSpacing:
                                                          temporary ? 2 : 0))),
                                          if (temporary)
                                            IconButton(
                                                iconSize: 19,
                                                tooltip: t('显示／隐藏验证码',
                                                    'Show / hide password'),
                                                onPressed: () => setState(
                                                    () => _visible = !_visible),
                                                icon: Icon(_visible
                                                    ? Icons.visibility_outlined
                                                    : Icons
                                                        .visibility_off_outlined)),
                                          if (temporary)
                                            IconButton(
                                                iconSize: 19,
                                                tooltip: t('刷新验证码',
                                                    'Refresh password'),
                                                onPressed: widget.onRefresh,
                                                icon:
                                                    const Icon(Icons.refresh)),
                                          IconButton(
                                              iconSize: 19,
                                              tooltip: t('验证设置',
                                                  'Authentication settings'),
                                              onPressed: widget.onSecurity,
                                              icon: const Icon(
                                                  Icons.settings_outlined)),
                                        ]),
                                        caption(temporary
                                            ? t('一次性密码，每次远控结束后自动更新',
                                                'One-time password, renewed after each session')
                                            : t('连接时按安全设置验证',
                                                'Uses your security settings')),
                                      ])),
                              OutlinedButton(
                                  onPressed: !widget.enabled
                                      ? null
                                      : () async {
                                          final passwordLine = temporary
                                              ? '\n${t('验证码', 'Password')}: ${widget.password}'
                                              : '';
                                          final text =
                                              'OpenUU\nID: ${widget.deviceId}$passwordLine';
                                          await Clipboard.setData(
                                              ClipboardData(text: text));
                                          if (mounted) {
                                            setState(() => _copied = true);
                                          }
                                        },
                                  child: Text(_copied
                                      ? t('已复制', 'Copied')
                                      : t('复制并分享', 'Copy and share'))),
                            ])),
                    const SizedBox(height: 20),
                    card(
                        Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(t('远控伙伴设备', 'Connect to a partner'),
                                  style: const TextStyle(
                                      fontSize: 18,
                                      fontWeight: FontWeight.w600)),
                              const SizedBox(height: 6),
                              caption(t('通过设备 ID 连接，并按对方设置完成验证',
                                  'Connect using a device ID, then authenticate with your partner.')),
                            ]),
                        Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              caption(t('伙伴的设备 ID', 'Partner device ID')),
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
                                            onChanged: (_) => setState(() {}),
                                            onSubmitted: (id) {
                                              if (id.trim().isNotEmpty) {
                                                widget.onConnect(id.trim());
                                              }
                                            },
                                            decoration: InputDecoration(
                                                hintText: t('请输入设备 ID',
                                                    'Enter device ID'),
                                                filled: true,
                                                fillColor: Colors.white,
                                                isDense: true,
                                                contentPadding:
                                                    const EdgeInsets.symmetric(
                                                        horizontal: 12,
                                                        vertical: 13),
                                                border: fieldStyle,
                                                enabledBorder: fieldStyle))),
                                    SizedBox(
                                        width: 140,
                                        height: 42,
                                        child: ElevatedButton(
                                            style: ElevatedButton.styleFrom(
                                                backgroundColor:
                                                    DesktopWelcomePage.blue,
                                                foregroundColor: Colors.white,
                                                elevation: 0,
                                                shape: RoundedRectangleBorder(
                                                    borderRadius: BorderRadius
                                                        .circular(5))),
                                            onPressed:
                                                _remoteId.text.trim().isEmpty
                                                    ? null
                                                    : () => widget.onConnect(
                                                        _remoteId.text.trim()),
                                            child: Text(t('连接', 'Connect')))),
                                  ]),
                            ])),
                  ]));
        }));
  }
}
