import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../desktop_welcome_page.dart';

part 'this_device_card.dart';
part 'partner_card.dart';

class DesktopAssistancePage extends StatefulWidget {
  final String deviceId, deviceName, password, verification, verificationMethod;
  final bool enabled, temporaryPassword, online;
  final Future<void> Function(bool) onEnable;
  final ValueChanged<String> onConnect, onVerificationChanged;
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
      required this.onRefresh,
      required this.onSecurity,
      required this.onDevices,
      required this.onFavorites,
      required this.onSettings,
      required this.onAccount});
  @override
  State<DesktopAssistancePage> createState() => _DesktopAssistancePageState();
}

const _muted = Color(0xff7b8492);

// Values of the verification-method option, as the settings page writes them.
const _useTemporaryPassword = 'use-temporary-password';
const _usePermanentPassword = 'use-permanent-password';
const _useBothPasswords = 'use-both-passwords';

Widget _caption(String text) =>
    Text(text, style: const TextStyle(fontSize: 13, color: _muted));

Widget _card(Widget heading, Widget content) => Container(
    decoration: BoxDecoration(
        color: Colors.white,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: const Color(0xffdce2e7))),
    child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Padding(padding: const EdgeInsets.all(18), child: heading),
      const Divider(height: 1, color: Color(0xffe3e7eb)),
      Padding(padding: const EdgeInsets.all(18), child: content),
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
        header: Text(t('开始协助', 'Start assistance'),
            style: const TextStyle(fontSize: 28, fontWeight: FontWeight.w600)),
        content: LayoutBuilder(builder: (context, bounds) {
          final inset = (bounds.maxWidth * .055).clamp(20.0, 48.0);
          return SingleChildScrollView(
              padding: EdgeInsets.fromLTRB(inset, 20, inset, 32),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    _thisDeviceCard(t),
                    const SizedBox(height: 20),
                    _partnerCard(t),
                  ]));
        }));
  }
}
