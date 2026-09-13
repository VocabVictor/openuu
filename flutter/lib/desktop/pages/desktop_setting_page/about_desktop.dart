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
  final valueStyle = UiType.caption.copyWith(color: UiColor.textSecondary);

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
                    icon: const Icon(Icons.content_copy_outlined,
                        color: UiColor.muted),
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
                style: valueStyle.copyWith(color: UiColor.primary));
        return _settingRow(
            context,
            'Version',
            Row(mainAxisSize: MainAxisSize.min, children: [
              status,
              const SizedBox(width: UiSpace.settingsControlGap),
              url.isEmpty
                  ? _secondaryButton('Check for updates',
                      () => bind.mainGetSoftwareUpdateUrl())
                  : _secondaryButton(
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
                          style: UiType.sectionTitle.copyWith(fontSize: 16)),
                      Text('v$version', style: UiType.caption),
                    ]),
              ]))),
      versionRow(),
      copyRow('Build Date', buildDate),
      copyRow('ID', myId),
      copyRow('Fingerprint', fingerprint),
      SettingsRow(
          label: translate('Website'),
          onTap: () => launchUrlString('https://github.com/VocabVictor/openuu'),
          control: const Icon(Icons.open_in_new, size: 14, color: UiColor.muted)),
    ]),
    const SizedBox(height: UiSpace.s3),
    SelectableText(
        'Copyright © ${DateTime.now().year} Purslane Tech Pte. Ltd.\n$license',
        style: UiType.caption.copyWith(color: UiColor.faint)),
  ]).marginOnly(bottom: _kListViewBottomMargin);
}
