part of 'desktop_tab_page.dart';

/// The home tab's page: the assistance page driven by the server model.
extension _AssistanceHome on _DesktopTabPageState {
  Widget _assistanceHome(BuildContext context) => buildRemoteBlock(
      block: _block,
      mask: true,
      use: canBeBlocked,
      child: AnimatedBuilder(
          animation: gFFI.serverModel,
          builder: (context, _) {
            final model = gFFI.serverModel;
            return DesktopAssistancePage(
                deviceId: model.serverId.text,
                password: model.serverPasswd.text,
                deviceName: Platform.localHostname,
                online: model.connectStatus > 0,
                verification: model.approveMode == 'click'
                    ? translate('Accept sessions via click')
                    : translate(_verificationLabel(model.verificationMethod)),
                verificationMethod: model.verificationMethod.isEmpty
                    ? kUseBothPasswords
                    : model.verificationMethod,
                onVerificationChanged: (method) =>
                    model.setVerificationMethod(method),
                temporaryPassword: model.approveMode != 'click' &&
                    model.verificationMethod != kUsePermanentPassword,
                enabled: !svcStopped.value,
                onEnable: (value) => start_service(value),
                onConnect: (id) => connect(context, id),
                recentPeers: gFFI.recentPeersModel.peers,
                onOpenRecent: (peer) => connect(context, peer.id),
                onRefresh: () => bind.mainUpdateTemporaryPassword(),
                onSecurity: () =>
                    DesktopSettingPage.switch2page(SettingsTabKey.safety),
                onDevices: () => DesktopTabPage.showHome(),
                onFavorites: () => DesktopTabPage.showHome(favorites: true),
                onSettings: DesktopTabPage.onAddSetting,
                onAccount: () => loginDialog());
          }));

  static String _verificationLabel(String method) {
    switch (method) {
      case kUseTemporaryPassword:
        return 'Use one-time password';
      case kUsePermanentPassword:
        return 'Use permanent password';
      default:
        return 'Use both passwords';
    }
  }
}

/// Keeps the home tab's page alive in the tab page view, so typed input on
/// the assistance page survives switching tabs.
class _KeepAlive extends StatefulWidget {
  final Widget child;
  const _KeepAlive({super.key, required this.child});
  @override
  State<_KeepAlive> createState() => _KeepAliveState();
}

class _KeepAliveState extends State<_KeepAlive>
    with AutomaticKeepAliveClientMixin {
  @override
  bool get wantKeepAlive => true;

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return widget.child;
  }
}
