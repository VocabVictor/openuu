import 'package:flutter/material.dart';

class TerminalSessionEntry {
  final String key, name;
  final bool connected;
  const TerminalSessionEntry({required this.key, required this.name, required this.connected});
}

class TerminalSessionsPage extends StatelessWidget {
  final String device;
  final List<TerminalSessionEntry> sessions;
  final VoidCallback onCreate, onMinimize, onClose, onDrag;
  final ValueChanged<String> onOpen, onRemove;
  const TerminalSessionsPage({super.key, required this.device, required this.sessions,
    required this.onCreate, required this.onMinimize, required this.onClose,
    required this.onDrag, required this.onOpen, required this.onRemove});

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    final connected = sessions.any((s) => s.connected);
    return Material(color: const Color(0xfffbfcfe), child: DefaultTextStyle(
      style: const TextStyle(fontFamily: 'Microsoft YaHei', fontFamilyFallback: ['Segoe UI'],
        fontSize: 14, color: Color(0xff18212b)),
      child: Column(children: [
        Container(height: 48, decoration: const BoxDecoration(gradient: LinearGradient(
          colors: [Color(0xfff3f9ff), Color(0xfff7f6fc)])),
          child: Row(children: [
            Expanded(child: GestureDetector(behavior: HitTestBehavior.opaque, onPanStart: (_) => onDrag(),
              child: Padding(padding: const EdgeInsets.symmetric(horizontal: 24), child: Align(
                alignment: Alignment.centerLeft, child: Text(t('终端远控', 'Remote terminal')))))),
            _windowButton(t('最小化', 'Minimize'), Icons.remove, onMinimize),
            _windowButton(t('关闭', 'Close'), Icons.close, onClose), const SizedBox(width: 8),
          ])),
        Expanded(child: LayoutBuilder(builder: (context, bounds) {
          final inset = bounds.maxWidth < 650 ? 20.0 : 28.0;
          return ListView(padding: EdgeInsets.fromLTRB(inset, 28, inset, 24), children: [
            Wrap(alignment: WrapAlignment.spaceBetween, crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 16, runSpacing: 16, children: [
                SizedBox(width: (bounds.maxWidth - inset * 2 - 210).clamp(180.0, 650.0), child: Row(children: [
                  Flexible(child: Text(device, maxLines: 1, overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontSize: 20, fontWeight: FontWeight.w600))),
                  const SizedBox(width: 14), Icon(Icons.circle, size: 9,
                    color: connected ? const Color(0xff16cf86) : const Color(0xff929aa5)),
                  const SizedBox(width: 7), Text(connected ? t('已连接', 'Connected') : t('连接中', 'Connecting'),
                    style: const TextStyle(fontSize: 13, color: Color(0xff737d89))),
                ])),
                _button(t('创建新的终端会话', 'Create terminal session'), onCreate),
              ]),
            const SizedBox(height: 22),
            if (sessions.isEmpty) Padding(padding: const EdgeInsets.all(32),
              child: Center(child: Text(t('暂无终端会话', 'No terminal sessions')))),
            for (final session in sessions) Container(
              margin: const EdgeInsets.only(bottom: 12), padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 17),
              decoration: BoxDecoration(color: Colors.white, border: Border.all(color: const Color(0xffdce0e5)), borderRadius: BorderRadius.circular(5)),
              child: Row(children: [
                Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text(session.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 16)),
                  const SizedBox(height: 7),
                  Text(session.connected ? t('创建时间未提供', 'Creation time unavailable') : t('正在建立会话…', 'Establishing session…'),
                    style: const TextStyle(fontSize: 12, color: Color(0xff8a939d))),
                ])),
                const SizedBox(width: 12),
                _button(t('打开终端', 'Open terminal'), session.connected ? () => onOpen(session.key) : null),
                const SizedBox(width: 12),
                PopupMenuButton<String>(tooltip: t('更多', 'More'), icon: const Icon(Icons.more_horiz, size: 20),
                  onSelected: (_) => onRemove(session.key), itemBuilder: (_) => [
                    PopupMenuItem(value: 'close', child: Text(t('关闭会话标签', 'Close session tab')))]),
              ])),
          ]);
        })),
      ])));
  }

  Widget _button(String text, VoidCallback? callback) => OutlinedButton(onPressed: callback,
    style: OutlinedButton.styleFrom(foregroundColor: const Color(0xff18212b), backgroundColor: Colors.white,
      side: const BorderSide(color: Color(0xffdce0e5)), shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(4)),
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 15)), child: Text(text));
  Widget _windowButton(String label, IconData icon, VoidCallback callback) => Tooltip(message: label,
    child: SizedBox.square(dimension: 36, child: InkWell(onTap: callback, borderRadius: BorderRadius.circular(4),
      child: Icon(icon, size: 18))));
}
