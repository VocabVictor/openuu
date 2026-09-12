part of 'tabbar_widget.dart';

extension _DesktopTabPageView on _DesktopTabState {
  Widget _buildPageView() {
    if (isWindows && !bind.isIncomingOnly() && tabType == DesktopTabType.main) {
      // Match the shell's selected state in the same frame, rather than exposing
      // the previous page while jumpTo waits for its PageController to attach.
      return Obx(() {
        final tabs = state.value.tabs;
        final selected = state.value.selected;
        return IndexedStack(
          index: selected,
          sizing: StackFit.expand,
          children: [
            for (var index = 0; index < tabs.length; index++)
              TickerMode(
                key: ValueKey(tabs[index].key),
                enabled: index == selected,
                child: tabs[index].page,
              ),
          ],
        );
      });
    }
    final child = Container(
        child: Obx(() => PageView(
            controller: state.value.pageController,
            physics: NeverScrollableScrollPhysics(),
            children: () {
              if (DesktopTabType.cm == tabType) {
                // Fix when adding a new tab still showing closed tabs with the same peer id, which would happen after the DesktopTab was stateful.
                return state.value.tabs.map((tab) {
                  return tab.page;
                }).toList();
              }

              /// to-do refactor, separate connection state and UI state for remote session.
              /// [workaround] PageView children need an immutable list, after it has been passed into PageView
              final tabLen = state.value.tabs.length;
              if (tabLen == _tabWidgets.length) {
                return _tabWidgets;
              } else if (_tabWidgets.isNotEmpty &&
                  tabLen == _tabWidgets.length + 1) {
                /// On add. Use the previous list(pointer) to prevent item's state init twice.
                /// *[_tabWidgets.isNotEmpty] means TabsWindow(remote_tab_page or file_manager_tab_page) opened before, but was hidden. In this case, we have to reload, otherwise the child can't be built.
                _tabWidgets.add(state.value.tabs.last.page);
                return _tabWidgets;
              } else {
                /// On remove or change. Use new list(pointer) to reload list children so that items loading order is normal.
                /// the Widgets in list must enable [AutomaticKeepAliveClientMixin]
                final newList = state.value.tabs.map((v) => v.page).toList();
                _tabWidgets = newList;
                return newList;
              }
            }())));
    if (tabType == DesktopTabType.remoteScreen) {
      return Container(color: kColorCanvas, child: child);
    } else {
      return child;
    }
  }

  /// Check whether to show ListView
  ///
  /// Conditions:
  /// - hide single item when only has one item (home) on [DesktopTabPage].
  bool isHideSingleItem() {
    return state.value.tabs.length == 1 &&
        (controller.tabType == DesktopTabType.main ||
            controller.tabType == DesktopTabType.install);
  }
}

extension _DesktopTabBar on _DesktopTabState {
  Widget _buildBar() {
    final isIncomingHomePage = bind.isIncomingOnly() && isInHomePage();
    return Row(
      children: [
        Expanded(
            child: GestureDetector(
                // custom double tap handler
                onTap: !isIncomingHomePage && showMaximize
                    ? () {
                        final current = DateTime.now().millisecondsSinceEpoch;
                        final elapsed = current - _lastClickTime;
                        _lastClickTime = current;
                        if (elapsed < bind.getDoubleClickTime()) {
                          // onDoubleTap
                          toggleMaximize(isMainWindow)
                              .then((value) => stateGlobal.setMaximized(value));
                        }
                      }
                    : (isIncomingHomePage ? () {} : null), // Keep tap recognizer for Windows touch.
                onPanStart: (_) => startDragging(isMainWindow),
                onPanCancel: () {
                  // We want to disable dragging of the tab area in the tab bar.
                  // Disable dragging is needed because macOS handles dragging by default.
                  if (isMacOS) {
                    setMovable(isMainWindow, false);
                  }
                },
                onPanEnd: (_) {
                  if (isMacOS) {
                    setMovable(isMainWindow, false);
                  }
                },
                child: Row(
                  children: [
                    Offstage(
                        offstage: !isMacOS,
                        child: const SizedBox(
                          width: 78,
                        )),
                    Offstage(
                      offstage: kUseCompatibleUiMode || isMacOS,
                      child: Row(children: [
                        Offstage(
                          offstage: !showLogo,
                          child: loadIcon(16),
                        ),
                        Offstage(
                            offstage: !showTitle,
                            child: const Text(
                              "OpenUU",
                              style: TextStyle(fontSize: 13),
                            ).marginOnly(left: 2))
                      ]).marginOnly(
                        left: 5,
                        right: 10,
                      ),
                    ),
                    Expanded(
                        child: Listener(
                            // handle mouse wheel
                            onPointerSignal: (e) {
                              if (e is PointerScrollEvent) {
                                final sc =
                                    controller.state.value.scrollController;
                                if (!sc.canScroll) return;
                                _scrollDebounce.call(() {
                                  double adjust = 2.5;
                                  sc.animateTo(
                                      sc.offset + e.scrollDelta.dy * adjust,
                                      duration: Duration(milliseconds: 200),
                                      curve: Curves.ease);
                                });
                              }
                            },
                            child: _ListView(
                              controller: controller,
                              invisibleTabKeys: invisibleTabKeys,
                              tabBuilder: tabBuilder,
                              tabMenuBuilder: tabMenuBuilder,
                              labelGetter: labelGetter,
                              maxLabelWidth: maxLabelWidth,
                              selectedTabBackgroundColor:
                                  selectedTabBackgroundColor,
                              unSelectedTabBackgroundColor:
                                  unSelectedTabBackgroundColor,
                              selectedBorderColor: selectedBorderColor,
                            ))),
                  ],
                ))),
        // hide simulated action buttons when we in compatible ui mode, because of reusing system title bar.
        WindowActionPanel(
          isMainWindow: isMainWindow,
          state: state,
          tabController: controller,
          invisibleTabKeys: invisibleTabKeys,
          tail: tail,
          showMinimize: showMinimize,
          showMaximize: showMaximize,
          showClose: showClose,
          onClose: onWindowCloseButton,
          labelGetter: labelGetter,
        ).paddingOnly(left: 10)
      ],
    );
  }
}
