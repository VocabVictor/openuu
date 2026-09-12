part of 'tabbar_widget.dart';

class _ListView extends StatelessWidget {
  final DesktopTabController controller;
  final RxList<String> invisibleTabKeys;

  final TabBuilder? tabBuilder;
  final TabMenuBuilder? tabMenuBuilder;
  final LabelGetter? labelGetter;
  final double? maxLabelWidth;
  final Color? selectedTabBackgroundColor;
  final Color? selectedBorderColor;
  final Color? unSelectedTabBackgroundColor;

  Rx<DesktopTabState> get state => controller.state;

  _ListView({
    required this.controller,
    required this.invisibleTabKeys,
    this.tabBuilder,
    this.tabMenuBuilder,
    this.labelGetter,
    this.maxLabelWidth,
    this.selectedTabBackgroundColor,
    this.unSelectedTabBackgroundColor,
    this.selectedBorderColor,
  });

  /// Check whether to show ListView
  ///
  /// Conditions:
  /// - hide single item when only has one item (home) on [DesktopTabPage].
  bool isHideSingleItem() {
    return state.value.tabs.length == 1 &&
            controller.tabType == DesktopTabType.main ||
        controller.tabType == DesktopTabType.install;
  }

  onVisibilityChanged(VisibilityInfo info) {
    final key = (info.key as ValueKey).value;
    if (info.visibleFraction < 0.75) {
      if (!invisibleTabKeys.contains(key)) {
        invisibleTabKeys.add(key);
      }
      invisibleTabKeys.removeWhere((key) =>
          controller.state.value.tabs.where((e) => e.key == key).isEmpty);
    } else {
      invisibleTabKeys.remove(key);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Obx(() => ListView(
        controller: state.value.scrollController,
        scrollDirection: Axis.horizontal,
        shrinkWrap: true,
        physics: const BouncingScrollPhysics(),
        children: isHideSingleItem()
            ? List.empty()
            : state.value.tabs.asMap().entries.map((e) {
                final index = e.key;
                final tab = e.value;
                final label = labelGetter == null
                    ? Rx<String>(tab.label)
                    : labelGetter!(tab.label);
                final child = VisibilityDetector(
                  key: ValueKey(tab.key),
                  onVisibilityChanged: onVisibilityChanged,
                  child: _Tab(
                    key: ValueKey(tab.key),
                    index: index,
                    tabInfoKey: tab.key,
                    label: label,
                    tabType: controller.tabType,
                    selectedIcon: tab.selectedIcon,
                    unselectedIcon: tab.unselectedIcon,
                    closable: tab.closable,
                    selected: state.value.selected,
                    onClose: () {
                      if (tab.onTabCloseButton != null) {
                        tab.onTabCloseButton!();
                      } else {
                        controller.remove(index);
                      }
                    },
                    onTap: () {
                      controller.jumpTo(index);
                      tab.onTap?.call();
                    },
                    tabBuilder: tabBuilder,
                    tabMenuBuilder: tabMenuBuilder,
                    maxLabelWidth: maxLabelWidth,
                    selectedTabBackgroundColor: selectedTabBackgroundColor ??
                        MyTheme.tabbar(context).selectedTabBackgroundColor,
                    unSelectedTabBackgroundColor: unSelectedTabBackgroundColor,
                    selectedBorderColor: selectedBorderColor,
                  ),
                );
                return GestureDetector(
                  onPanStart: (e) {},
                  child: child,
                );
              }).toList()));
  }
}

class _Tab extends StatefulWidget {
  final int index;
  final String tabInfoKey;
  final Rx<String> label;
  final DesktopTabType tabType;
  final IconData? selectedIcon;
  final IconData? unselectedIcon;
  final bool closable;
  final int selected;
  final Function() onClose;
  final Function() onTap;
  final TabBuilder? tabBuilder;
  final TabMenuBuilder? tabMenuBuilder;
  final double? maxLabelWidth;
  final Color? selectedTabBackgroundColor;
  final Color? unSelectedTabBackgroundColor;
  final Color? selectedBorderColor;

  const _Tab({
    Key? key,
    required this.index,
    required this.tabInfoKey,
    required this.label,
    required this.tabType,
    this.selectedIcon,
    this.unselectedIcon,
    this.tabBuilder,
    this.tabMenuBuilder,
    required this.closable,
    required this.selected,
    required this.onClose,
    required this.onTap,
    this.maxLabelWidth,
    this.selectedTabBackgroundColor,
    this.unSelectedTabBackgroundColor,
    this.selectedBorderColor,
  }) : super(key: key);

  @override
  State<_Tab> createState() => _TabState();
}
