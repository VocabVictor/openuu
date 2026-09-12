part of 'peer_tab_page.dart';

extension _PeerTabBars on _PeerTabPageState {
  Widget _createSwitchBar(BuildContext context) {
    final model = Provider.of<PeerTabModel>(context);
    var counter = -1;
    return ReorderableListView(
        buildDefaultDragHandles: false,
        onReorder: model.reorder,
        scrollDirection: Axis.horizontal,
        physics: NeverScrollableScrollPhysics(),
        children: model.visibleEnabledOrderedIndexs.map((t) {
          final selected = model.currentTab == t;
          final color = selected
              ? MyTheme.tabbar(context).selectedTextColor
              : MyTheme.tabbar(context).unSelectedTextColor
            ?..withOpacity(0.5);
          final hover = false.obs;
          final deco = BoxDecoration(
              color: Theme.of(context).colorScheme.background,
              borderRadius: BorderRadius.circular(6));
          final decoBorder = BoxDecoration(
              border: Border(
            bottom: BorderSide(width: 2, color: color!),
          ));
          counter += 1;
          return ReorderableDragStartListener(
              key: ValueKey(t),
              index: counter,
              child: Obx(() => Tooltip(
                    preferBelow: false,
                    message: model.tabTooltip(t),
                    onTriggered: isMobile ? mobileShowTabVisibilityMenu : null,
                    child: InkWell(
                      child: Container(
                        decoration: (hover.value
                            ? (selected ? decoBorder : deco)
                            : (selected ? decoBorder : null)),
                        child: Icon(model.tabIcon(t), color: color)
                            .paddingSymmetric(horizontal: 4),
                      ).paddingSymmetric(horizontal: 4),
                      onTap: isOptionFixed(kOptionPeerTabIndex)
                          ? null
                          : () async {
                              await handleTabSelection(t);
                              await bind.setLocalFlutterOption(
                                  k: kOptionPeerTabIndex, v: t.toString());
                            },
                      onHover: (value) => hover.value = value,
                    ),
                  )));
        }).toList());
  }

  Widget _createPeersView() {
    final model = Provider.of<PeerTabModel>(context);
    Widget child;
    if (model.visibleEnabledOrderedIndexs.isEmpty) {
      child = visibleContextMenuListener(Row(
        children: [Expanded(child: InkWell())],
      ));
    } else {
      if (model.visibleEnabledOrderedIndexs.contains(model.currentTab)) {
        child = entries[model.currentTab].widget;
      } else {
        debugPrint("should not happen! currentTab not in visibleIndexs");
        Future.delayed(Duration.zero, () {
          model.setCurrentTab(model.visibleEnabledOrderedIndexs[0]);
        });
        child = entries[0].widget;
      }
    }
    return Expanded(
        child: child.marginSymmetric(
            vertical: (isDesktop || isWebDesktop) ? 12.0 : 6.0));
  }

  Widget _createRefresh(
      {required PeerTabIndex index, required RxBool loading}) {
    final model = Provider.of<PeerTabModel>(context);
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    return Offstage(
      offstage: model.currentTab != index.index,
      child: Tooltip(
        message: translate('Refresh'),
        child: RefreshWidget(
            onPressed: () {
              if (gFFI.peerTabModel.currentTab < entries.length) {
                entries[gFFI.peerTabModel.currentTab].load?.call();
              }
            },
            spinning: loading,
            child: RotatedBox(
                quarterTurns: 2,
                child: Icon(
                  Icons.refresh,
                  size: 18,
                  color: textColor,
                ))),
      ),
    );
  }

  Widget _createPeerViewTypeSwitch(BuildContext context) {
    return PeerViewDropdown();
  }

  Widget _createMultiSelection() {
    final textColor = Theme.of(context).textTheme.titleLarge?.color;
    final model = Provider.of<PeerTabModel>(context);
    return _hoverAction(
      toolTip: translate('Select'),
      context: context,
      onTap: () {
        model.setMultiSelectionMode(true);
        if (isMobile && Navigator.canPop(context)) {
          Navigator.pop(context);
        }
      },
      child: SvgPicture.asset(
        "assets/checkbox-outline.svg",
        width: 18,
        height: 18,
        colorFilter: svgColor(textColor),
      ),
    );
  }
}
