part of 'desktop_setting_page.dart';

const double _kTabWidth = 200;

const double _kTabHeight = 42;

const double _kCardFixedWidth = 540;

const double _kCardLeftMargin = 15;

const double _kContentHMargin = 15;

const double _kContentHSubMargin = _kContentHMargin + 33;

const double _kCheckBoxLeftMargin = 10;

const double _kRadioLeftMargin = 10;

const double _kListViewBottomMargin = 15;

const double _kTitleFontSize = 20;

const double _kContentFontSize = 15;

const Color _accentColor = MyTheme.accent;

const String _kSettingPageControllerTag = 'settingPageController';

const String _kSettingPageTabKeyTag = 'settingPageTabKey';

class _TabInfo {
  late final SettingsTabKey key;
  late final String label;
  late final IconData unselected;
  late final IconData selected;
  _TabInfo(this.key, this.label, this.unselected, this.selected);
}

enum SettingsTabKey {
  general,
  safety,
  network,
  display,
  account,
  about,
}
