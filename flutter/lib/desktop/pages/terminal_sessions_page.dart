import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/ui_palette.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';

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
    final pal = UiColor.of(context);
    final type = UiType.of(context);
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    String t(String cn, String en) => zh ? cn : en;
    final connected = sessions.any((s) => s.connected);
    return Material(color: pal.panelBg, child: DefaultTextStyle(
      style: type.rowTitle.copyWith(fontWeight: FontWeight.w400),
      child: Column(children: [
        Container(height: UiSession.tabBarHeight, decoration: BoxDecoration(
          color: pal.surface,
          border: Border(bottom: BorderSide(color: pal.border))),
          child: Row(children: [
            Expanded(child: GestureDetector(behavior: HitTestBehavior.opaque, onPanStart: (_) => onDrag(),
              child: Padding(padding: const EdgeInsets.symmetric(horizontal: UiSession.tabPaddingX), child: Align(
                alignment: Alignment.centerLeft, child: Text(t('终端远控', 'Remote terminal'),
                  style: type.sidebarItem.copyWith(color: pal.text)))))),
            _windowButton(pal, t('最小化', 'Minimize'), Icons.remove, onMinimize),
            _windowButton(pal, t('关闭', 'Close'), Icons.close, onClose),
            const SizedBox(width: UiSpace.s2),
          ])),
        Expanded(child: LayoutBuilder(builder: (context, bounds) {
          final inset = bounds.maxWidth < 650 ? 20.0 : 28.0;
          return ListView(padding: EdgeInsets.fromLTRB(inset, 28, inset, 24), children: [
            Wrap(alignment: WrapAlignment.spaceBetween, crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 16, runSpacing: 16, children: [
                SizedBox(width: (bounds.maxWidth - inset * 2 - 210).clamp(180.0, 650.0), child: Row(children: [
                  Flexible(child: Text(device, maxLines: 1, overflow: TextOverflow.ellipsis,
                    style: type.pageTitle)),
                  const SizedBox(width: UiSpace.s3),
                  Icon(Icons.circle, size: UiSpace.statusDotSize,
                    color: connected ? pal.success : pal.muted),
                  const SizedBox(width: UiSpace.statusDotGap),
                  Text(connected ? t('已连接', 'Connected') : t('连接中', 'Connecting'),
                    style: type.caption.copyWith(color: pal.textSecondary)),
                ])),
                _button(pal, type,
                    t('创建新的终端会话', 'Create terminal session'), onCreate),
              ]),
            const SizedBox(height: 22),
            if (sessions.isEmpty) Padding(padding: const EdgeInsets.all(UiSpace.s8),
              child: Center(child: Text(t('暂无终端会话', 'No terminal sessions'),
                style: type.caption))),
            for (final session in sessions) Container(
              margin: const EdgeInsets.only(bottom: UiSpace.rowCardGap),
              padding: const EdgeInsets.symmetric(
                horizontal: UiSpace.sectionCardPadding, vertical: UiSpace.s3),
              decoration: BoxDecoration(color: pal.surface,
                border: Border.all(color: pal.border),
                borderRadius: BorderRadius.circular(UiSpace.rowCardRadius)),
              child: Row(children: [
                Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Text(session.name, maxLines: 1, overflow: TextOverflow.ellipsis,
                    style: type.rowTitle),
                  const SizedBox(height: UiSpace.s1),
                  Text(session.connected ? t('创建时间未提供', 'Creation time unavailable') : t('正在建立会话…', 'Establishing session…'),
                    style: type.caption),
                ])),
                const SizedBox(width: UiSpace.s3),
                _button(pal, type, t('打开终端', 'Open terminal'),
                    session.connected ? () => onOpen(session.key) : null),
                const SizedBox(width: UiSpace.s2),
                PopupMenuButton<String>(tooltip: t('更多', 'More'),
                  icon: Icon(Icons.more_horiz, size: UiSpace.rowActionIconSize,
                    color: pal.muted),
                  onSelected: (_) => onRemove(session.key), itemBuilder: (_) => [
                    PopupMenuItem(value: 'close', child: Text(t('关闭会话标签', 'Close session tab')))]),
              ])),
          ]);
        })),
      ])));
  }

  Widget _button(UiPalette pal, UiTypeset type, String text,
          VoidCallback? callback) =>
      SizedBox(
    height: UiSpace.controlHeight,
    child: OutlinedButton(onPressed: callback,
      style: OutlinedButton.styleFrom(foregroundColor: pal.text, backgroundColor: pal.surface,
        side: BorderSide(color: pal.inputBorder),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(UiSpace.buttonRadius)),
        padding: const EdgeInsets.symmetric(horizontal: UiSpace.s3),
        textStyle: type.button), child: Text(text)));
  Widget _windowButton(UiPalette pal, String label, IconData icon,
          VoidCallback callback) =>
      Tooltip(message: label,
    child: SizedBox.square(dimension: UiSession.tabActionHitSize,
      child: InkWell(onTap: callback, borderRadius: BorderRadius.circular(UiSpace.buttonRadius),
        child: Icon(icon, size: UiSession.tabActionIconSize,
          color: pal.textSecondary))));
}
