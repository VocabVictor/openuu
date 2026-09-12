part of 'peer_tab_page.dart';

extension _PeerTabRightActions on _PeerTabPageState {
  List<Widget> _landscapeRightActions(BuildContext context) {
    final model = Provider.of<PeerTabModel>(context);
    return [
      const PeerSearchBar().marginOnly(right: 13),
      _createRefresh(
          index: PeerTabIndex.ab, loading: gFFI.abModel.currentAbLoading),
      _createRefresh(
          index: PeerTabIndex.group, loading: gFFI.groupModel.groupLoading),
      Offstage(
        offstage: model.currentTabCachedPeers.isEmpty,
        child: _createMultiSelection(),
      ),
      _createPeerViewTypeSwitch(context),
      Offstage(
        offstage: model.currentTab == PeerTabIndex.recent.index,
        child: PeerSortDropdown(),
      ),
      Offstage(
        offstage: model.currentTab != PeerTabIndex.ab.index,
        child: _toggleTags(),
      ),
    ];
  }

  List<Widget> _portraitRightActions(BuildContext context) {
    final model = Provider.of<PeerTabModel>(context);
    final screenWidth = MediaQuery.of(context).size.width;
    final leftIconSize = Theme.of(context).iconTheme.size ?? 24;
    final leftActionsSize =
        (leftIconSize + (4 + 4) * 2) * model.visibleEnabledOrderedIndexs.length;
    final availableWidth = screenWidth - 10 * 2 - leftActionsSize - 2 * 2;
    final searchWidth = 120;
    final otherActionWidth = 18 + 10;

    dropDown(List<Widget> menus) {
      final padding = 6.0;
      final textColor = Theme.of(context).textTheme.titleLarge?.color;
      return PullDownButton(
        buttonBuilder:
            (BuildContext context, Future<void> Function() showMenu) {
          return _hoverAction(
            context: context,
            toolTip: translate('More'),
            child: SvgPicture.asset(
              "assets/chevron_up_chevron_down.svg",
              width: 18,
              height: 18,
              colorFilter: svgColor(textColor),
            ),
            onTap: showMenu,
          );
        },
        routeTheme: PullDownMenuRouteTheme(
            width: menus.length * (otherActionWidth + padding * 2) * 1.0),
        itemBuilder: (context) => [
          PullDownMenuEntryImpl(
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: menus
                  .map((e) =>
                      Material(child: e.paddingSymmetric(horizontal: padding)))
                  .toList(),
            ),
          )
        ],
      );
    }

    // Always show search, refresh
    List<Widget> actions = [
      const PeerSearchBar(),
      if (model.currentTab == PeerTabIndex.ab.index)
        _createRefresh(
            index: PeerTabIndex.ab, loading: gFFI.abModel.currentAbLoading),
      if (model.currentTab == PeerTabIndex.group.index)
        _createRefresh(
            index: PeerTabIndex.group, loading: gFFI.groupModel.groupLoading),
    ];
    final List<Widget> dynamicActions = [
      if (model.currentTabCachedPeers.isNotEmpty) _createMultiSelection(),
      if (model.currentTab != PeerTabIndex.recent.index) PeerSortDropdown(),
      if (model.currentTab == PeerTabIndex.ab.index) _toggleTags()
    ];
    final rightWidth = availableWidth -
        searchWidth -
        (actions.length == 2 ? otherActionWidth : 0);
    final availablePositions = rightWidth ~/ otherActionWidth;

    if (availablePositions < dynamicActions.length &&
        dynamicActions.length > 1) {
      if (availablePositions < 2) {
        actions.addAll([
          dropDown(dynamicActions),
        ]);
      } else {
        actions.addAll([
          ...dynamicActions.sublist(0, availablePositions - 1),
          dropDown(dynamicActions.sublist(availablePositions - 1)),
        ]);
      }
    } else {
      actions.addAll(dynamicActions);
    }
    return actions;
  }
}
