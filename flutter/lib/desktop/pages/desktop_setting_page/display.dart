part of 'desktop_setting_page.dart';

class _Display extends StatefulWidget {
  const _Display({Key? key}) : super(key: key);

  @override
  State<_Display> createState() => _DisplayState();
}

class _DisplayState extends State<_Display> {
  void _setState(VoidCallback fn) => setState(fn);
  @override
  Widget build(BuildContext context) {
    final scrollController = ScrollController();
    if (isWindows && !bind.isIncomingOnly()) {
      final zh = Localizations.localeOf(context).languageCode == 'zh';
      return ListView(controller: scrollController, children: [
        _group(zh ? '新建连接的默认值' : 'Defaults for new connections', [
          viewStyle(context),
          scrollStyle(context),
          imageQuality(context),
          codec(context),
          privacyModeImpl(context),
        ]),
        trackpadSpeed(context),
        other(context),
      ]).marginOnly(bottom: _kListViewBottomMargin);
    }
    return ListView(controller: scrollController, children: [
      viewStyle(context),
      scrollStyle(context),
      imageQuality(context),
      codec(context),
      if (isDesktop) trackpadSpeed(context),
      privacyModeImpl(context),
      other(context),
    ]).marginOnly(bottom: _kListViewBottomMargin);
  }

}

extension _DisplayOther on _DisplayState {
  Widget privacyModeImpl(BuildContext context) {
    final supportedPrivacyModeImpls = bind.mainSupportedPrivacyModeImpls();
    late final List<dynamic> privacyModeImpls;
    try {
      privacyModeImpls = jsonDecode(supportedPrivacyModeImpls);
    } catch (e) {
      debugPrint('failed to parse supported privacy mode impls, err=$e');
      return Offstage();
    }
    if (privacyModeImpls.length < 2) {
      return Offstage();
    }

    final key = 'privacy-mode-impl-key';
    onChanged(String value) async {
      await bind.mainSetOption(key: key, value: value);
      _setState(() {});
    }

    String groupValue = bind.mainGetOptionSync(key: key);
    if (groupValue.isEmpty) {
      groupValue = bind.mainDefaultPrivacyModeImpl();
    }
    if (isWindows && !bind.isIncomingOnly()) {
      return _choiceCard(context, 'Privacy mode', groupValue, {
        for (final impl in privacyModeImpls)
          (impl as List<dynamic>)[0] as String: impl[1] as String,
      }, onChanged);
    }
    return _Card(
      title: 'Privacy mode',
      children: privacyModeImpls.map((impl) {
        final d = impl as List<dynamic>;
        return _Radio(context,
            value: d[0] as String,
            groupValue: groupValue,
            label: d[1] as String,
            onChanged: onChanged);
      }).toList(),
    );
  }

  Widget otherRow(String label, String key) {
    final value = getOtherDefaultSettingOption(key) == 'Y';
    final isOptFixed = isOtherDefaultSettingReadOnly(key);
    onChanged(bool b) async {
      await setOtherDefaultSettingOption(
        key,
        b ? 'Y' : (key == kOptionEnableFileCopyPaste ? 'N' : defaultOptionNo),
      );
      _setState(() {});
    }

    if (isWindows && !bind.isIncomingOnly()) {
      return _settingRow(
          context,
          label,
          SettingsSwitch(value: value, onChanged: isOptFixed ? null : onChanged),
          enabled: !isOptFixed);
    }
    return GestureDetector(
        child: Row(
          children: [
            Checkbox(
                    value: value,
                    onChanged: isOptFixed ? null : (_) => onChanged(!value))
                .marginOnly(right: 5),
            Expanded(
              child: Text(translate(label)),
            )
          ],
        ).marginOnly(left: _kCheckBoxLeftMargin),
        onTap: isOptFixed ? null : () => onChanged(!value));
  }

  Widget other(BuildContext context) {
    final children =
        otherDefaultSettings().map((e) => otherRow(e.$1, e.$2)).toList();
    if (isWindows && !bind.isIncomingOnly()) {
      final zh = Localizations.localeOf(context).languageCode == 'zh';
      return _group(zh ? '高级' : 'Advanced', children, collapsible: true);
    }
    return _Card(title: 'Other Default Options', children: children);
  }
}
