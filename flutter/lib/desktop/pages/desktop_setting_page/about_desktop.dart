part of 'desktop_setting_page.dart';

/// The About tab on the desktop shell (design-review-settings.md §2.7): one
/// card with a brand head, the version row with its update state, copyable
/// read-only rows and the website link; the copyright sits under the card.
Widget _aboutDesktop(BuildContext context,
    {required String version,
    required String buildDate,
    required String fingerprint,
    required String myId,
    required String license}) {
  final zh = Localizations.localeOf(context).languageCode == 'zh';
  final valueStyle = UiType.of(context).caption.copyWith(color: UiColor.of(context).textSecondary);

  Widget copyRow(String label, String value) => value.isEmpty
      ? const Offstage()
      : _settingRow(
          context,
          label,
          Row(mainAxisSize: MainAxisSize.min, children: [
            Text(value, style: valueStyle),
            const SizedBox(width: UiSpace.settingsControlGap),
            SizedBox(
                width: UiSpace.settingsSwitchHitHeight,
                height: UiSpace.settingsSwitchHitHeight,
                child: IconButton(
                    padding: EdgeInsets.zero,
                    iconSize: 14,
                    tooltip: translate('Copy'),
                    icon: Icon(Icons.content_copy_outlined,
                        color: UiColor.of(context).muted),
                    onPressed: () async {
                      await Clipboard.setData(ClipboardData(text: value));
                      showToast(zh ? '已复制' : 'Copied');
                    })),
          ]),
          description: '');

  Widget versionRow() => Obx(() {
        final url = stateGlobal.updateUrl.value;
        final installed = bind.mainIsInstalled();
        final status = url.isEmpty
            ? Text(translate('Up to date'), style: valueStyle)
            : Text(
                '${translate("new-version-of-{${bind.mainGetAppNameSync()}}-tip")} (${bind.mainGetNewVersion()})',
                style: valueStyle.copyWith(color: UiColor.of(context).primary));
        return _settingRow(
            context,
            'Version',
            Row(mainAxisSize: MainAxisSize.min, children: [
              status,
              const SizedBox(width: UiSpace.settingsControlGap),
              url.isEmpty
                  ? _secondaryButton(context, 'Check for updates',
                      () => bind.mainGetSoftwareUpdateUrl())
                  : _secondaryButton(context, 
                      installed ? 'Update' : 'Download',
                      () => installed
                          ? handleUpdate(url)
                          : launchUrlString(url)),
            ]),
            description: '');
      });

  return ListView(children: [
    _group(null, [
      SizedBox(
          height: UiSpace.settingsBrandHeadHeight,
          child: Padding(
              padding: const EdgeInsets.symmetric(
                  horizontal: UiSpace.settingsRowPaddingX),
              child: Row(children: [
                const BrandIcon(size: UiSpace.settingsBrandIconSize),
                const SizedBox(width: UiSpace.s3),
                Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('OpenUU',
                          style: UiType.of(context).sectionTitle.copyWith(fontSize: 16)),
                      Text('v$version', style: UiType.of(context).caption),
                    ]),
              ]))),
      versionRow(),
      copyRow('Build Date', buildDate),
      copyRow('ID', myId),
      SettingsRow(
          label: zh ? '开源许可' : 'Open source licence',
          onTap: () => _licenceDialog(zh),
          control: Icon(Icons.chevron_right,
              size: 16, color: UiColor.of(context).muted)),
      SettingsRow(
          label: translate('Website'),
          onTap: () => launchUrlString('https://github.com/VocabVictor/openuu'),
          control: Icon(Icons.open_in_new, size: 14, color: UiColor.of(context).muted)),
    ]),
    if (fingerprint.isNotEmpty)
      _group(zh ? '高级' : 'Advanced', [copyRow('Fingerprint', fingerprint)],
          collapsible: true),
    const SizedBox(height: UiSpace.s3),
    SelectableText(
        'Copyright © ${DateTime.now().year} Purslane Tech Pte. Ltd.\n$license',
        style: UiType.of(context).caption.copyWith(color: UiColor.of(context).faint)),
  ]).marginOnly(bottom: _kListViewBottomMargin);
}

/// OpenUU is a fork of RustDesk and stays under the GNU AGPL v3; the notice
/// must be reachable from the app.
void _licenceDialog(bool zh) {
  const url = 'https://github.com/VocabVictor/openuu/blob/master/LICENCE';
  gFFI.dialogManager.show((setState, close, context) => CustomAlertDialog(
      titlePadding: EdgeInsets.zero,
      contentBoxConstraints: const BoxConstraints(
          minWidth: UiSpace.dialogContentWidth,
          maxWidth: UiSpace.dialogContentWidth),
      content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _dialogTitle(context, zh ? '开源许可' : 'Open source licence', close),
            const SizedBox(height: UiSpace.s2),
            Text(
                zh ? 'OpenUU 基于 RustDesk 开发，遵循 GNU Affero General Public License v3.0 发布。你可以按该许可的条款使用、修改和分发本软件，完整许可文本与源代码见下方链接。' : 'OpenUU is derived from RustDesk and is released under the GNU Affero General Public License v3.0. You may use, modify and redistribute it under that licence; the full text and the source code are behind the link below.',
                style: UiType.of(context).rowTitle
                    .copyWith(fontWeight: FontWeight.w400, height: 1.5)),
            const SizedBox(height: UiSpace.s6),
            Row(mainAxisAlignment: MainAxisAlignment.end, children: [
              _secondaryButton(context, zh ? '查看许可全文' : 'View licence',
                  () => launchUrlString(url),
                  height: UiSpace.controlHeight),
              const SizedBox(width: UiSpace.s2),
              _primaryButton(context, zh ? '关闭' : 'Close', close,
                  height: UiSpace.controlHeight),
            ]),
          ]),
      onCancel: close));
}
