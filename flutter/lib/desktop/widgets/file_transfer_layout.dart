import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/ui_tokens.dart';

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
                          style: UiType.pageTitle))),
              const SizedBox(width: UiSpace.s2),
              Container(
                  height: UiSpace.tagHeight,
                  alignment: Alignment.center,
                  padding: const EdgeInsets.symmetric(
                      horizontal: UiSpace.tagPaddingX),
                  decoration: BoxDecoration(
                      color: local
                          ? UiColor.settingsRowHover
                          : UiColor.primaryTint,
                      borderRadius:
                          BorderRadius.circular(UiSpace.tagRadius)),
                  child: Text(
                      local ? (zh ? '本机' : 'Local') : (zh ? '远端' : 'Remote'),
                      style: UiType.tag.copyWith(
                          color: local
                              ? UiColor.textSecondary
                              : UiColor.primary))),
            ]));
    Widget panel(Widget child) => Container(
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
            color: dark ? const Color(0xff22262c) : Colors.white,
            border: Border.all(
                color: dark ? Colors.white24 : UiColor.border),
            borderRadius:
                BorderRadius.circular(UiSpace.sectionCardRadius)),
        child: child);
    final theme = Theme.of(context);
    return Theme(
        data: theme.copyWith(
          textTheme: theme.textTheme.apply(fontFamily: 'Microsoft YaHei'),
          colorScheme: theme.colorScheme.copyWith(primary: UiColor.primary),
          filledButtonTheme: FilledButtonThemeData(
              style: FilledButton.styleFrom(
                  backgroundColor: UiColor.primary,
                  foregroundColor: Colors.white,
                  disabledBackgroundColor: UiColor.primaryDisabled,
                  disabledForegroundColor: Colors.white,
                  elevation: 0,
                  textStyle: UiType.button,
                  shape: RoundedRectangleBorder(
                      borderRadius:
                          BorderRadius.circular(UiSpace.buttonRadius)),
                  minimumSize:
                      const Size(84, UiSpace.controlHeight))),
        ),
        child: ColoredBox(
            color: dark ? const Color(0xff191d23) : UiColor.panelBg,
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
                              icon: const Icon(Icons.arrow_forward, size: 16),
                              label: Text(zh ? '发送' : 'Send')),
                          const SizedBox(width: UiSpace.s3),
                          FilledButton.icon(
                              onPressed: onReceive,
                              icon: const Icon(Icons.arrow_back, size: 16),
                              label: Text(zh ? '接收' : 'Receive')),
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
