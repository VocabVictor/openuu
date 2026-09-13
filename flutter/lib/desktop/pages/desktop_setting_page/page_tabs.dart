part of 'desktop_setting_page.dart';

extension _DesktopSettingPageTabs on _DesktopSettingPageState {
  List<_TabInfo> _settingTabs() {
    final List<_TabInfo> settingTabs = <_TabInfo>[];
    for (final tab in DesktopSettingPage.tabKeys) {
      switch (tab) {
        case SettingsTabKey.general:
          settingTabs.add(_TabInfo(
              tab, 'General', Icons.settings_outlined, Icons.settings));
          break;
        case SettingsTabKey.safety:
          settingTabs.add(_TabInfo(tab, 'Security',
              Icons.enhanced_encryption_outlined, Icons.enhanced_encryption));
          break;
        case SettingsTabKey.network:
          settingTabs
              .add(_TabInfo(tab, 'Network', Icons.link_outlined, Icons.link));
          break;
        case SettingsTabKey.display:
          settingTabs.add(_TabInfo(tab, 'Display',
              Icons.desktop_windows_outlined, Icons.desktop_windows));
          break;
        case SettingsTabKey.account:
          settingTabs.add(
              _TabInfo(tab, 'Account', Icons.person_outline, Icons.person));
          break;
        case SettingsTabKey.about:
          settingTabs
              .add(_TabInfo(tab, 'About', Icons.info_outline, Icons.info));
          break;
      }
    }
    return settingTabs;
  }

  List<Widget> _children() {
    final children = List<Widget>.empty(growable: true);
    for (final tab in DesktopSettingPage.tabKeys) {
      switch (tab) {
        case SettingsTabKey.general:
          children.add(const _General());
          break;
        case SettingsTabKey.safety:
          children.add(const _Safety());
          break;
        case SettingsTabKey.network:
          children.add(const _Network());
          break;
        case SettingsTabKey.display:
          children.add(const _Display());
          break;
        case SettingsTabKey.account:
          children.add(const _Account());
          break;
        case SettingsTabKey.about:
          children.add(const _About());
          break;
      }
    }
    return children;
  }

  Widget _buildBlock({required List<Widget> children}) {
    // check both mouseMoveTime and videoConnCount
    return Obx(() {
      final videoConnBlock =
          _canBeBlocked.value && stateGlobal.videoConnCount > 0;
      return Stack(children: [
        buildRemoteBlock(
          block: _block,
          mask: false,
          use: canBeBlocked,
          child: preventMouseKeyBuilder(
            child: Row(children: children),
            block: videoConnBlock,
          ),
        ),
        if (videoConnBlock)
          Container(
            color: Colors.black.withOpacity(0.5),
          )
      ]);
    });
  }
}

extension _DesktopSettingPageList on _DesktopSettingPageState {
  Widget _header(BuildContext context) {
    final settingsText = Text(
      translate('Settings'),
      textAlign: TextAlign.left,
      style: const TextStyle(
        color: _accentColor,
        fontSize: _kTitleFontSize,
        fontWeight: FontWeight.w400,
      ),
    );
    return Row(
      children: [
        SizedBox(
            height: 62,
            child: settingsText,
          ).marginOnly(left: 20, top: 10),
        const Spacer(),
      ],
    );
  }

  Widget _listView({required List<_TabInfo> tabs}) {
    final scrollController = ScrollController();
    return ListView(
      controller: scrollController,
      children: tabs.map((tab) => _listItem(tab: tab)).toList(),
    );
  }

  Widget _listItem({required _TabInfo tab}) {
    return Obx(() {
      bool selected = tab.key == selectedTab.value;
      return SizedBox(
        width: _kTabWidth,
        height: _kTabHeight,
        child: InkWell(
          onTap: () {
            if (selectedTab.value != tab.key) {
              int index = DesktopSettingPage.tabKeys.indexOf(tab.key);
              if (index == -1) {
                return;
              }
              controller.jumpToPage(index);
            }
            selectedTab.value = tab.key;
          },
          child: Row(children: [
            Container(
              width: 4,
              height: _kTabHeight * 0.7,
              color: selected ? _accentColor : null,
            ),
            Icon(
              selected ? tab.selected : tab.unselected,
              color: selected ? _accentColor : null,
              size: 20,
            ).marginOnly(left: 13, right: 10),
            Text(
              translate(tab.label),
              style: TextStyle(
                  color: selected ? _accentColor : null,
                  fontWeight: FontWeight.w400,
                  fontSize: _kContentFontSize),
            ),
          ]),
        ),
      );
    });
  }
}
