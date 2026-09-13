part of 'desktop_assistance_page.dart';

extension _ThisDeviceCard on _DesktopAssistancePageState {
  Widget _thisDeviceCard(String Function(String, String) t) {
    final temporary = widget.temporaryPassword;
    return _card(
        Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: 20,
            runSpacing: 8,
            children: [
              Row(mainAxisSize: MainAxisSize.min, children: [
                Text(t('本设备', 'This device'),
                    style: const TextStyle(
                        fontSize: 18, fontWeight: FontWeight.w600)),
                const SizedBox(width: 14),
                Text(widget.deviceName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 14, color: _muted)),
                const SizedBox(width: 10),
                Icon(Icons.circle,
                    size: 8,
                    color: widget.online
                        ? const Color(0xff18dfa0)
                        : Colors.grey),
                const SizedBox(width: 5),
                Text(
                    widget.online
                        ? t('在线', 'Online')
                        : t('未就绪', 'Not ready'),
                    style: const TextStyle(fontSize: 13, color: _muted)),
              ]),
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
                            _setState(() => _busy = true);
                            try {
                              await widget.onEnable(value);
                            } finally {
                              if (mounted) {
                                _setState(() => _busy = false);
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
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        _caption(t('本设备 ID', 'This device ID')),
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
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        PopupMenuButton<String>(
                            tooltip: t('切换验证方式', 'Change verification'),
                            initialValue: widget.verificationMethod,
                            onSelected: widget.onVerificationChanged,
                            itemBuilder: (_) => [
                                  PopupMenuItem(
                                      value: _useTemporaryPassword,
                                      child: Text(t('仅使用一次性密码',
                                          'One-time password only'))),
                                  PopupMenuItem(
                                      value: _usePermanentPassword,
                                      child: Text(t('仅使用固定密码',
                                          'Permanent password only'))),
                                  PopupMenuItem(
                                      value: _useBothPasswords,
                                      child: Text(t('同时使用两种密码',
                                          'Use both passwords'))),
                                ],
                            child: Row(children: [
                              Expanded(child: _caption(widget.verification)),
                              const Icon(Icons.expand_more,
                                  size: 17, color: _muted)
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
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                      fontSize: temporary ? 22 : 14,
                                      letterSpacing: temporary ? 2 : 0))),
                          if (temporary)
                            IconButton(
                                iconSize: 19,
                                tooltip: t('显示／隐藏验证码',
                                    'Show / hide password'),
                                onPressed: () =>
                                    _setState(() => _visible = !_visible),
                                icon: Icon(_visible
                                    ? Icons.visibility_outlined
                                    : Icons.visibility_off_outlined)),
                          if (temporary)
                            IconButton(
                                iconSize: 19,
                                tooltip: t('刷新验证码', 'Refresh password'),
                                onPressed: widget.onRefresh,
                                icon: const Icon(Icons.refresh)),
                          IconButton(
                              iconSize: 19,
                              tooltip: t('验证设置', 'Authentication settings'),
                              onPressed: widget.onSecurity,
                              icon: const Icon(Icons.settings_outlined)),
                        ]),
                        _caption(temporary
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
                          await Clipboard.setData(ClipboardData(text: text));
                          if (mounted) {
                            _setState(() => _copied = true);
                          }
                        },
                  child: Text(_copied
                      ? t('已复制', 'Copied')
                      : t('复制并分享', 'Copy and share'))),
            ]));
  }
}
