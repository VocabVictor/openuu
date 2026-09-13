import 'package:flutter/material.dart';
import '../../common.dart';
import '../../models/wol_model.dart';
import 'ui_tokens.dart';

class DeviceActionBar extends StatefulWidget {
  final String id;
  final VoidCallback onFiles, onWatch, onTerminal, onTunnel;

  const DeviceActionBar({super.key, required this.id, required this.onFiles,
    required this.onWatch, required this.onTerminal, required this.onTunnel});

  @override
  State<DeviceActionBar> createState() => _DeviceActionBarState();
}

class _DeviceActionBarState extends State<DeviceActionBar> {
  final _order = [0, 1, 2, 3];
  static const _icons = [Icons.folder_open_outlined, Icons.ondemand_video,
    Icons.terminal, Icons.settings_ethernet, Icons.power_settings_new,
    Icons.restart_alt, Icons.power_settings_new];

  List<String> get _labels => [
    translate('Files'), translate('View only'), translate('Terminal'),
    translate('Port forwarding'), translate('Wake up'), translate('Restart'),
    translate('Shut down'),
  ];

  List<VoidCallback> get _actions => [widget.onFiles, widget.onWatch,
    widget.onTerminal, widget.onTunnel];

  Future<void> _showTools() async {
    final ui = UiColor.of(context);
    final labels = _labels;
    var editing = false;
    var wakeState = 'unavailable';
    try { wakeState = (await WolModel.request('status', {'id': widget.id}))['state'] as String; }
    catch (_) { /* Older servers do not expose wake support. */ }
    if (!mounted) return;
    final canWake = wakeState == 'available';
    final wakeReason = wakeState == 'unregistered'
      ? translate('Sign in on the target device first')
      : wakeState == 'no_helper'
        ? translate('No online LAN helper has discovered this device')
        : translate('Wake service unavailable');
    final selected = await showDialog<int>(context: context, builder: (dialogContext) =>
      StatefulBuilder(builder: (context, updateDialog) => Dialog(
        backgroundColor: ui.surface,
        surfaceTintColor: Colors.transparent,
        insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        child: SizedBox(width: 560, child: ConstrainedBox(
          constraints: BoxConstraints(maxHeight: MediaQuery.sizeOf(context).height - 48),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Padding(padding: const EdgeInsets.all(24), child: Row(children: [
              Expanded(child: Text(translate('More tools'),
                style: TextStyle(fontSize: 22, color: ui.text))),
              OutlinedButton(onPressed: () => updateDialog(() => editing = !editing),
                child: Text(editing ? translate('Done') : translate('Reorder'))),
            ])),
            Divider(height: 1, color: ui.border),
            Flexible(child: SingleChildScrollView(padding: const EdgeInsets.all(24),
              child: Column(children: [
                for (final tool in [..._order, if (wakeState != 'online') 4, 5, 6])
                  Padding(padding: const EdgeInsets.only(bottom: 5), child: Material(
                    color: tool < 4 ? ui.surfaceHover : ui.panelBg,
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(4),
                      side: BorderSide(color: tool < 4 ? ui.border : ui.settingsDivider)),
                    child: ListTile(
                      key: ValueKey('tool-$tool'),
                      enabled: tool < 4 || (tool == 4 && canWake),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
                      leading: Icon(_icons[tool], size: 26),
                      title: Text(labels[tool], style: const TextStyle(fontSize: 16)),
                      subtitle: tool >= 4 ? Text(tool == 4 ? (canWake ? translate('Wake through an online LAN device') : wakeReason) : translate('Not available for this device'),
                        style: const TextStyle(fontSize: 12)) : null,
                      trailing: editing && tool < 4 ? Row(mainAxisSize: MainAxisSize.min, children: [
                        IconButton(tooltip: translate('Move up'), icon: const Icon(Icons.arrow_upward, size: 18),
                          onPressed: _order.indexOf(tool) == 0 ? null : () {
                            updateDialog(() { final i = _order.indexOf(tool); _order.removeAt(i); _order.insert(i - 1, tool); });
                            setState(() {});
                          }),
                        IconButton(tooltip: translate('Move down'), icon: const Icon(Icons.arrow_downward, size: 18),
                          onPressed: _order.indexOf(tool) == 3 ? null : () {
                            updateDialog(() { final i = _order.indexOf(tool); _order.removeAt(i); _order.insert(i + 1, tool); });
                            setState(() {});
                          }),
                      ]) : const Icon(Icons.chevron_right, size: 20),
                      onTap: (tool < 4 || (tool == 4 && canWake)) && !editing ? () => Navigator.pop(dialogContext, tool) : null,
                    ),
                  )),
              ]))),
            Container(padding: const EdgeInsets.all(24),
              decoration: BoxDecoration(color: ui.panelBg,
                border: Border(top: BorderSide(color: ui.border))),
              child: Align(alignment: Alignment.centerRight, child: SizedBox(width: 248,
                child: OutlinedButton(onPressed: () => Navigator.pop(dialogContext),
                  child: Text(translate('Close')))))),
          ]),
        )),
      )));
    if (!mounted || selected == null) return;
    if (selected < 4) { _actions[selected](); return; }
    var message = translate('Wake requested. Wait for the device to come online; WOL and standby power are required.');
    try { await WolModel.request('wake', {'id': widget.id}); }
    catch (_) { message = translate('Wake request failed. Check your login and LAN helper.'); }
    if (mounted) await showDialog<void>(context: context, builder: (context) => AlertDialog(
      title: Text(translate('Wake up')), content: Text(message),
      actions: [TextButton(onPressed: () => Navigator.pop(context), child: Text(translate('Close')))],
    ));
  }

  @override
  Widget build(BuildContext context) {
    final ui = UiColor.of(context);
    return LayoutBuilder(builder: (context, constraints) {
      final compact = constraints.maxWidth < 560;
      Widget action(String label, IconData icon, VoidCallback callback) => Expanded(
        child: Tooltip(message: label, child: TextButton(
          onPressed: callback,
          style: TextButton.styleFrom(
            foregroundColor: ui.text,
            minimumSize: const Size(0, 52),
            padding: const EdgeInsets.symmetric(horizontal: 8),
            shape: const RoundedRectangleBorder(),
          ),
          child: compact ? Icon(icon, size: 19) : FittedBox(fit: BoxFit.scaleDown,
            child: Row(mainAxisSize: MainAxisSize.min, children: [
              Icon(icon, size: 19), const SizedBox(width: 8), Text(label),
            ])),
        )),
      );
      Widget divider() => SizedBox(height: 22,
        child: VerticalDivider(width: 1, thickness: 1, color: ui.border));
      return Container(
        height: 52,
        decoration: BoxDecoration(gradient: LinearGradient(
          begin: Alignment.topCenter, end: Alignment.bottomCenter,
          colors: [ui.panelBg, ui.surface],
        )),
        child: Row(children: [
          for (final tool in _order) ...[
            action(_labels[tool], _icons[tool], _actions[tool]), divider(),
          ],
          SizedBox(width: 52, height: 52, child: IconButton(
            tooltip: translate('More'),
            icon: Icon(Icons.grid_view_outlined, size: 20, color: ui.text),
            onPressed: () => _showTools(),
          )),
        ]),
      );
    });
  }
}
