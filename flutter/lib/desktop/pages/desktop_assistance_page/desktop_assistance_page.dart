import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';
import '../../../common.dart';
import '../../../models/peer_model.dart';
import '../../../models/platform_model.dart';
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

Widget _caption(BuildContext context, String text) =>
    Text(text, style: UiType.of(context).caption);

/// A section card: a 48-high header row, a divider, then the content on the
/// section padding. `divider: false` keeps title and content in one block.
Widget _card(BuildContext context, Widget heading, Widget content,
        {bool divider = true}) =>
    Container(
        decoration: BoxDecoration(
            color: UiColor.of(context).surface,
            borderRadius: BorderRadius.circular(UiSpace.sectionCardRadius),
            border: Border.all(color: UiColor.of(context).border)),
        child:
            Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          Container(
              constraints: const BoxConstraints(
                  minHeight: UiSpace.sectionCardHeaderHeight),
              padding: const EdgeInsets.symmetric(
                  horizontal: UiSpace.sectionCardPadding),
              alignment: Alignment.centerLeft,
              child: heading),
          if (divider)
            Divider(height: 1, color: UiColor.of(context).border),
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
    return DesktopWelcomePage(
        assistanceSelected: true,
        onLogin: widget.onAccount,
        onDevices: widget.onDevices,
        onAssistance: () {},
        onFavorites: widget.onFavorites,
        onSettings: widget.onSettings,
        header: Text(translate('Start assistance'),
            style: UiType.of(context).pageTitle),
        content: SingleChildScrollView(
            padding: const EdgeInsets.fromLTRB(UiSpace.pagePaddingX, 0,
                UiSpace.pagePaddingX, UiSpace.pagePaddingBottom),
            child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _thisDeviceCard(),
                  const SizedBox(height: UiSpace.sectionCardGap),
                  _partnerCard(),
                  const SizedBox(height: UiSpace.sectionCardGap),
                  _recentCard(),
                ])));
  }
}
