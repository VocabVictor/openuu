part of 'desktop_setting_page.dart';

extension _GeneralOther on _GeneralState {
  Widget other() {
    final incomingOnly = bind.isIncomingOnly();
    final outgoingOnly = bind.isOutgoingOnly();
    final showAutoUpdate = (isWindows && bind.mainIsInstalled()) ||
        (isMacOS &&
            bind.mainIsInstalled() &&
            bind.mainIsInstalledDaemon(prompt: false) &&
            !bind.isCustomClient());
    final children = <Widget>[
      if (!isWeb && !incomingOnly)
        _OptionCheckBox(context, 'Confirm before closing multiple tabs',
            kOptionEnableConfirmClosingTabs,
            isServer: false),
      if (!incomingOnly)
        _OptionCheckBox(
          context,
          'allow-remote-toolbar-docking-any-edge',
          kOptionAllowMultiEdgeToolbarDock,
          isServer: false,
          update: (_) {
            reloadAllWindows();
          },
        ),
      if (!isWeb && !outgoingOnly)
        _OptionCheckBox(context, 'Adaptive bitrate', kOptionEnableAbr),
      if (!isWeb) wallpaper(),
      if (!isWeb && !incomingOnly) ...[
        _OptionCheckBox(
          context,
          'Open connection in new tab',
          kOptionOpenNewConnInTabs,
          isServer: false,
        ),
        Tooltip(
          message: translate('port-forward-mux-tip'),
          child: _OptionCheckBox(
            context,
            'Reuse one connection for port forwarding',
            kOptionEnablePortForwardMux,
            isServer: false,
          ),
        ),
        // though this is related to GUI, but opengl problem affects all users, so put in config rather than local
        if (isLinux)
          Tooltip(
            message: translate('software_render_tip'),
            child: _OptionCheckBox(
              context,
              "Always use software rendering",
              kOptionAllowAlwaysSoftwareRender,
            ),
          ),
        if (!isWeb)
          Tooltip(
            message: translate('texture_render_tip'),
            child: _OptionCheckBox(
              context,
              "Use texture rendering",
              kOptionTextureRender,
              optGetter: bind.mainGetUseTextureRender,
              optSetter: (k, v) async =>
                  await bind.mainSetLocalOption(key: k, value: v ? 'Y' : 'N'),
            ),
          ),
        if (isWindows)
          Tooltip(
            message: translate('d3d_render_tip'),
            child: _OptionCheckBox(
              context,
              "Use D3D rendering",
              kOptionD3DRender,
              isServer: false,
            ),
          ),
      ],
      if (!isWeb && !bind.isCustomClient())
        _OptionCheckBox(
          context,
          'Check for software update on startup',
          kOptionEnableCheckUpdate,
          isServer: false,
        ),
      if (showAutoUpdate)
        _OptionCheckBox(
          context,
          'Auto update',
          kOptionAllowAutoUpdate,
          isServer: true,
        ),
      if (isWindows && !outgoingOnly)
        _OptionCheckBox(
          context,
          'Capture screen using DirectX',
          kOptionDirectxCapture,
        ),
      if (!isWeb && !incomingOnly) ...[
        _OptionCheckBox(
          context,
          'Enable TCP hole punching',
          kOptionEnableTcpPunch,
          isServer: false,
        ),
        _OptionCheckBox(
          context,
          'Enable UDP hole punching',
          kOptionEnableUdpPunch,
          isServer: false,
        ),
        _OptionCheckBox(
          context,
          'Enable IPv6 P2P connection',
          kOptionEnableIpv6Punch,
          isServer: false,
        ),
      ],
      if (!incomingOnly)
        _OptionCheckBox(
          context,
          'Enable WebRTC P2P connection',
          kOptionEnableWebrtc,
          isServer: false,
        ),
      if (!isWeb && !incomingOnly)
        Tooltip(
          message: translate('sync-clipboard-between-sessions-tip'),
          child: _OptionCheckBox(
            context,
            'Sync clipboard between sessions',
            kOptionAllowSyncClipboardBetweenSessions,
            isServer: false,
          ),
        ),
    ];

    // Add client-side wakelock option for desktop platforms
    if (!bind.isIncomingOnly()) {
      children.add(_OptionCheckBox(
        context,
        'keep-awake-during-outgoing-sessions-label',
        kOptionKeepAwakeDuringOutgoingSessions,
        isServer: false,
      ));
    }

    if (!bind.isDisableAccount()) {
      children.add(_OptionCheckBox(
        context,
        'note-at-conn-end-tip',
        kOptionAllowAskForNoteAtEndOfConnection,
        isServer: false,
        optSetter: (key, value) async {
          if (value && !gFFI.userModel.isLogin) {
            final res = await loginDialog();
            if (res != true) return;
          }
          await mainSetLocalBoolOption(key, value);
        },
      ));
    }
    children.add(_OptionCheckBox(
      context,
      'Show monitor switch button on the main toolbar',
      kOptionAllowMonitorSwitchMainToolbar,
      isServer: false,
      update: (enabled) async {
        if (!enabled) {
          await mainSetLocalBoolOption(
              kOptionAllowMonitorSwitchMinToolbar, false);
        }
        if (mounted) _setState(() {});
        reloadAllWindows();
        if (enabled) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            final ctx = _minToolbarOptionKey.currentContext;
            if (ctx != null) {
              Scrollable.ensureVisible(
                ctx,
                alignment: 0.5,
                duration: const Duration(milliseconds: 250),
                curve: Curves.easeInOut,
              );
            }
          });
        }
      },
    ));
    if (mainGetLocalBoolOptionSync(kOptionAllowMonitorSwitchMainToolbar)) {
      children.add(KeyedSubtree(
        key: _minToolbarOptionKey,
        child: _OptionCheckBox(
          context,
          'Show on the minimized toolbar',
          kOptionAllowMonitorSwitchMinToolbar,
          isServer: false,
          update: (_) {
            reloadAllWindows();
          },
        ).marginOnly(left: _kCheckBoxLeftMargin * 3),
      ));
    }
    return _Card(title: 'Other', children: children);
  }
}
