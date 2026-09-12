part of 'peer_card.dart';

extension _BasePeerCardConnectActions on BasePeerCard {
  MenuEntryBase<String> _connectCommonAction(
    BuildContext context,
    String title, {
    bool isFileTransfer = false,
    bool isViewCamera = false,
    bool isTcpTunneling = false,
    bool isRDP = false,
    bool isTerminal = false,
    bool isTerminalRunAsAdmin = false,
  }) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        title,
        style: style,
      ),
      proc: () {
        if (isTerminalRunAsAdmin) {
          setEnvTerminalAdmin();
        }
        connectInPeerTab(
          context,
          peer,
          tab,
          isFileTransfer: isFileTransfer,
          isViewCamera: isViewCamera,
          isTcpTunneling: isTcpTunneling,
          isRDP: isRDP,
          isTerminal: isTerminal || isTerminalRunAsAdmin,
        );
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _connectAction(BuildContext context) {
    return _connectCommonAction(
      context,
      (peer.alias.isEmpty
          ? translate('Connect')
          : '${translate('Connect')} ${peer.id}'),
    );
  }

  @protected
  MenuEntryBase<String> _transferFileAction(BuildContext context) {
    return _connectCommonAction(
      context,
      translate('Transfer file'),
      isFileTransfer: true,
    );
  }

  @protected
  MenuEntryBase<String> _viewCameraAction(BuildContext context) {
    return _connectCommonAction(
      context,
      translate('View camera'),
      isViewCamera: true,
    );
  }

  @protected
  MenuEntryBase<String> _terminalAction(BuildContext context) {
    return _connectCommonAction(
      context,
      '${translate('Terminal')} (beta)',
      isTerminal: true,
    );
  }

  @protected
  MenuEntryBase<String> _terminalRunAsAdminAction(BuildContext context) {
    return _connectCommonAction(
      context,
      '${translate('Terminal (Run as administrator)')} (beta)',
      isTerminalRunAsAdmin: true,
    );
  }

  @protected
  MenuEntryBase<String> _tcpTunnelingAction(BuildContext context) {
    return _connectCommonAction(
      context,
      translate('TCP tunneling'),
      isTcpTunneling: true,
    );
  }

  @protected
  MenuEntryBase<String> _rdpAction(BuildContext context, String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Container(
          alignment: AlignmentDirectional.center,
          height: CustomPopupMenuTheme.height,
          child: Row(
            children: [
              Text(
                translate('RDP'),
                style: style,
              ),
              Expanded(
                  child: Align(
                alignment: Alignment.centerRight,
                child: Transform.scale(
                    scale: 0.8,
                    child: IconButton(
                      icon: const Icon(Icons.edit),
                      padding: EdgeInsets.zero,
                      onPressed: () {
                        if (Navigator.canPop(context)) {
                          Navigator.pop(context);
                        }
                        _rdpDialog(id);
                      },
                    )),
              ))
            ],
          )),
      proc: () {
        connectInPeerTab(context, peer, tab, isRDP: true);
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  @protected
  MenuEntryBase<String> _wolAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('WOL'),
        style: style,
      ),
      proc: () {
        bind.mainWol(id: id);
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  /// Only available on Windows.
  @protected
  MenuEntryBase<String> _createShortCutAction(String id) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Create desktop shortcut'),
        style: style,
      ),
      proc: () {
        bind.mainCreateShortcut(id: id);
        showToast(translate('Successful'));
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }

  Future<MenuEntryBase<String>> _openNewConnInAction(
      String id, String label, String key) async {
    return MenuEntrySwitch<String>(
      switchType: SwitchType.scheckbox,
      text: translate(label),
      getter: () async => mainGetPeerBoolOptionSync(id, key),
      setter: (bool v) async {
        await bind.mainSetPeerOption(
            id: id, key: key, value: bool2option(key, v));
        showToast(translate('Successful'));
      },
      padding: menuPadding,
      dismissOnClicked: true,
    );
  }
}
