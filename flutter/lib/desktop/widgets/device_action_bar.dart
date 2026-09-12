import '../../models/wol_model.dart';
import 'package:flutter/material.dart';

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

  List<String> _labels(bool zh) => zh
    ? ['文件传输', '观看模式', '终端', '端口映射', '远程开机', '重启', '关机']
    : ['Files', 'View only', 'Terminal', 'Port forwarding', 'Wake up', 'Restart', 'Shut down'];

  List<VoidCallback> get _actions => [widget.onFiles, widget.onWatch,
    widget.onTerminal, widget.onTunnel];

  Future<void> _showTools(bool zh) async {
    final labels = _labels(zh);
    var editing = false;
    var wakeState = 'unavailable';
    try { wakeState = (await WolModel.request('status', {'id': widget.id}))['state'] as String; }
    catch (_) { /* Older servers do not expose wake support. */ }
    if (!mounted) return;
    final canWake = wakeState == 'available';
    final wakeReason = wakeState == 'unregistered'
      ? (zh ? '请先在目标电脑登录一次' : 'Sign in on the target device first')
      : wakeState == 'no_helper'
        ? (zh ? '需要已发现目标的同账号在线局域网节点' : 'No online LAN helper has discovered this device')
        : (zh ? '唤醒服务暂不可用' : 'Wake service unavailable');
    final selected = await showDialog<int>(context: context, builder: (dialogContext) =>
      StatefulBuilder(builder: (context, updateDialog) => Dialog(
        backgroundColor: Colors.white,
        surfaceTintColor: Colors.transparent,
        insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        child: SizedBox(width: 560, child: ConstrainedBox(
          constraints: BoxConstraints(maxHeight: MediaQuery.sizeOf(context).height - 48),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Padding(padding: const EdgeInsets.all(24), child: Row(children: [
              Expanded(child: Text(zh ? '更多工具' : 'More tools',
                style: const TextStyle(fontSize: 22, color: Color(0xff20262d)))),
              OutlinedButton(onPressed: () => updateDialog(() => editing = !editing),
                child: Text(editing ? (zh ? '完成' : 'Done') : (zh ? '调整顺序' : 'Reorder'))),
            ])),
            const Divider(height: 1, color: Color(0xffd9dfe5)),
            Flexible(child: SingleChildScrollView(padding: const EdgeInsets.all(24),
              child: Column(children: [
                for (final tool in [..._order, if (wakeState != 'online') 4, 5, 6])
                  Padding(padding: const EdgeInsets.only(bottom: 5), child: Material(
                    color: tool < 4 ? const Color(0xffeff4f7) : const Color(0xfffafafa),
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(4),
                      side: BorderSide(color: tool < 4 ? const Color(0xffd3dbe1) : const Color(0xffeceeef))),
                    child: ListTile(
                      key: ValueKey('tool-$tool'),
                      enabled: tool < 4 || (tool == 4 && canWake),
                      contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
                      leading: Icon(_icons[tool], size: 26),
                      title: Text(labels[tool], style: const TextStyle(fontSize: 16)),
                      subtitle: tool >= 4 ? Text(tool == 4 ? (canWake ? (zh ? '由在线局域网设备代发唤醒包' : 'Wake through an online LAN device') : wakeReason) : (zh ? '当前设备暂不支持' : 'Not available for this device'),
                        style: const TextStyle(fontSize: 12)) : null,
                      trailing: editing && tool < 4 ? Row(mainAxisSize: MainAxisSize.min, children: [
                        IconButton(tooltip: zh ? '上移' : 'Move up', icon: const Icon(Icons.arrow_upward, size: 18),
                          onPressed: _order.indexOf(tool) == 0 ? null : () {
                            updateDialog(() { final i = _order.indexOf(tool); _order.removeAt(i); _order.insert(i - 1, tool); });
                            setState(() {});
                          }),
                        IconButton(tooltip: zh ? '下移' : 'Move down', icon: const Icon(Icons.arrow_downward, size: 18),
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
              decoration: const BoxDecoration(color: Color(0xfff7fafb),
                border: Border(top: BorderSide(color: Color(0xffd9dfe5)))),
              child: Align(alignment: Alignment.centerRight, child: SizedBox(width: 248,
                child: OutlinedButton(onPressed: () => Navigator.pop(dialogContext),
                  child: Text(zh ? '关闭' : 'Close'))))),
          ]),
        )),
      )));
    if (!mounted || selected == null) return;
    if (selected < 4) { _actions[selected](); return; }
    var message = zh ? '已请求发送唤醒包，请等待设备上线。目标需启用 WOL 并保持供电。' : 'Wake requested. Wait for the device to come online; WOL and standby power are required.';
    try { await WolModel.request('wake', {'id': widget.id}); }
    catch (_) { message = zh ? '唤醒请求失败，请检查登录和在线代发节点。' : 'Wake request failed. Check your login and LAN helper.'; }
    if (mounted) await showDialog<void>(context: context, builder: (context) => AlertDialog(
      title: Text(zh ? '远程开机' : 'Wake up'), content: Text(message),
      actions: [TextButton(onPressed: () => Navigator.pop(context), child: Text(zh ? '关闭' : 'Close'))],
    ));
  }

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    return LayoutBuilder(builder: (context, constraints) {
      final compact = constraints.maxWidth < 560;
      Widget action(String label, IconData icon, VoidCallback callback) => Expanded(
        child: Tooltip(message: label, child: TextButton(
          onPressed: callback,
          style: TextButton.styleFrom(
            foregroundColor: const Color(0xff243747),
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
      Widget divider() => const SizedBox(height: 22,
        child: VerticalDivider(width: 1, thickness: 1, color: Color(0xffd9dfe5)));
      return Container(
        height: 52,
        decoration: const BoxDecoration(gradient: LinearGradient(
          begin: Alignment.topCenter, end: Alignment.bottomCenter,
          colors: [Color(0xffedf0f7), Color(0xfffafcfe)],
        )),
        child: Row(children: [
          for (final tool in _order) ...[
            action(_labels(zh)[tool], _icons[tool], _actions[tool]), divider(),
          ],
          SizedBox(width: 52, height: 52, child: IconButton(
            tooltip: t('更多', 'More'),
            icon: const Icon(Icons.grid_view_outlined, size: 20, color: Color(0xff243747)),
            onPressed: () => _showTools(zh),
          )),
        ]),
      );
    });
  }
}
