import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../models/peer_model.dart';
import '../../widgets/device_row.dart';
import '../../widgets/ui_tokens.dart';
import '../desktop_welcome_page.dart';

part 'this_device_card.dart';
part 'partner_card.dart';
part 'recent_card.dart';

class DesktopAssistancePage extends StatefulWidget {
  final String deviceId, deviceName, password, verification, verificationMethod;
  final bool enabled, temporaryPassword, online;
  final Future<void> Function(bool) onEnable;
  final ValueChanged<String> onConnect, onVerificationChanged;
  /// Most recent sessions, newest first; tapping a card connects to it.
  final List<Peer> recentPeers;
  final ValueChanged<Peer> onOpenRecent;
  final VoidCallback onRefresh,
      onSecurity,
      onDevices,
      onFavorites,
      onSettings,
      onAccount;
  const DesktopAssistancePage(
      {super.key,
      required this.deviceId,
      required this.deviceName,
      required this.online,
      required this.password,
      required this.verification,
      required this.verificationMethod,
      required this.onVerificationChanged,
      required this.enabled,
      required this.temporaryPassword,
      required this.onEnable,
      required this.onConnect,
      required this.recentPeers,
      required this.onOpenRecent,
      required this.onRefresh,
      required this.onSecurity,
      required this.onDevices,
      required this.onFavorites,
      required this.onSettings,
      required this.onAccount});
  @override
  State<DesktopAssistancePage> createState() => _DesktopAssistancePageState();
}

// Values of the verification-method option, as the settings page writes them.
const _useTemporaryPassword = 'use-temporary-password';
const _usePermanentPassword = 'use-permanent-password';
const _useBothPasswords = 'use-both-passwords';

Widget _caption(String text) => Text(text, style: UiType.caption);

/// A section card: a 48-high header row, a divider, then the content on the
/// section padding. `divider: false` keeps title and content in one block.
Widget _card(Widget heading, Widget content, {bool divider = true}) =>
    Container(
        decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(UiSpace.sectionCardRadius),
            border: Border.all(color: UiColor.border)),
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Container(
              constraints: const BoxConstraints(
                  minHeight: UiSpace.sectionCardHeaderHeight),
              padding: const EdgeInsets.symmetric(
                  horizontal: UiSpace.sectionCardPadding),
              alignment: Alignment.centerLeft,
              child: heading),
          if (divider) const Divider(height: 1, color: UiColor.border),
          Padding(
              padding: EdgeInsets.fromLTRB(
                  UiSpace.sectionCardPadding,
                  divider ? UiSpace.sectionCardPadding : 0,
                  UiSpace.sectionCardPadding,
                  UiSpace.sectionCardPadding),
              child: content),
        ]));

class _DesktopAssistancePageState extends State<DesktopAssistancePage> {
  final _remoteId = TextEditingController();
  bool _visible = false, _busy = false, _copied = false;

  void _setState(VoidCallback fn) => setState(fn);

  @override
  void dispose() {
    _remoteId.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    return DesktopWelcomePage(
        assistanceSelected: true,
        onLogin: widget.onAccount,
        onDevices: widget.onDevices,
        onAssistance: () {},
        onFavorites: widget.onFavorites,
        onSettings: widget.onSettings,
        header: Text(t('开始协助', 'Start assistance'), style: UiType.pageTitle),
        content: SingleChildScrollView(
            padding: const EdgeInsets.fromLTRB(UiSpace.pagePaddingX, 0,
                UiSpace.pagePaddingX, UiSpace.pagePaddingBottom),
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _thisDeviceCard(t),
                  const SizedBox(height: UiSpace.sectionCardGap),
                  _partnerCard(t),
                  const SizedBox(height: UiSpace.sectionCardGap),
                  _recentCard(t),
                ])));
  }
}
