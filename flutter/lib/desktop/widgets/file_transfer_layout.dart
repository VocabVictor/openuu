import 'package:flutter/material.dart';

class FileTransferLayout extends StatelessWidget {
  final String localName, remoteName;
  final Widget localBrowser, remoteBrowser, transfers;
  final VoidCallback? onSend, onReceive;

  const FileTransferLayout(
      {super.key,
      required this.localName,
      required this.remoteName,
      required this.localBrowser,
      required this.remoteBrowser,
      required this.transfers,
      this.onSend,
      this.onReceive});

  @override
  Widget build(BuildContext context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final dark = Theme.of(context).brightness == Brightness.dark;
    Widget identity(String name, bool local) => Expanded(
            child: Row(
                mainAxisAlignment:
                    local ? MainAxisAlignment.start : MainAxisAlignment.end,
                children: [
              Flexible(
                  child: Tooltip(
                      message: name,
                      child: Text(name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 24, fontWeight: FontWeight.w600)))),
              const SizedBox(width: 10),
              Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
                  decoration: BoxDecoration(
                      color: local
                          ? const Color(0xffdce1e8)
                          : const Color(0xffb8efdc),
                      borderRadius: BorderRadius.circular(4)),
                  child: Text(
                      local ? (zh ? '本机' : 'Local') : (zh ? '远端' : 'Remote'),
                      style: const TextStyle(
                          fontSize: 12, color: Color(0xff365168)))),
            ]));
    Widget panel(Widget child) => Container(
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
            color: dark ? const Color(0xff22262c) : const Color(0xfff9fbfd),
            border: Border.all(
                color: dark ? Colors.white24 : const Color(0xffd8dde3)),
            borderRadius: BorderRadius.circular(6)),
        child: child);
    final theme = Theme.of(context);
    return Theme(
        data: theme.copyWith(
          textTheme: theme.textTheme.apply(fontFamily: 'Microsoft YaHei'),
          colorScheme:
              theme.colorScheme.copyWith(primary: const Color(0xff3979ff)),
          filledButtonTheme: FilledButtonThemeData(
              style: FilledButton.styleFrom(
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(4)),
                  minimumSize: const Size(84, 36))),
        ),
        child: ColoredBox(
            color: dark ? const Color(0xff191d23) : const Color(0xfff0f4f8),
            child: LayoutBuilder(builder: (context, bounds) {
              final inset = bounds.maxWidth < 1000 ? 16.0 : 24.0;
              return Padding(
                  padding: EdgeInsets.all(inset),
                  child: Column(children: [
                    SizedBox(
                        height: 52,
                        child: Row(children: [
                          identity(localName, true),
                          const SizedBox(width: 12),
                          FilledButton.icon(
                              onPressed: onSend,
                              icon: const Icon(Icons.arrow_forward, size: 17),
                              label: Text(zh ? '发送' : 'Send')),
                          const SizedBox(width: 16),
                          FilledButton.icon(
                              onPressed: onReceive,
                              icon: const Icon(Icons.arrow_back, size: 17),
                              label: Text(zh ? '发送' : 'Send')),
                          const SizedBox(width: 12),
                          identity(remoteName, false),
                        ])),
                    const SizedBox(height: 14),
                    Expanded(
                        flex: 6,
                        child: Row(children: [
                          Expanded(child: panel(localBrowser)),
                          SizedBox(width: inset),
                          Expanded(child: panel(remoteBrowser)),
                        ])),
                    const SizedBox(height: 22),
                    Expanded(flex: 3, child: panel(transfers)),
                  ]));
            })));
  }
}
