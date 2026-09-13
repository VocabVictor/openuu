part of 'tabbar_widget.dart';

/// Session windows (remote, file transfer, camera, port forward, terminal)
/// take the UiSession tab look; the main window, CM and installer keep the
/// TabbarTheme defaults.
bool _isSessionTab(DesktopTabType tabType) =>
    tabType != DesktopTabType.main &&
    tabType != DesktopTabType.cm &&
    tabType != DesktopTabType.install;

class _TabState extends State<_Tab> with RestorationMixin {
  final RestorableBool restoreHover = RestorableBool(false);

  Widget _buildTabContent() {
    bool showIcon =
        widget.selectedIcon != null && widget.unselectedIcon != null;
    bool isSelected = widget.index == widget.selected;
    final session = _isSessionTab(widget.tabType);
    final iconSize = session ? UiSession.tabIconSize : _kIconSize;

    final icon = Offstage(
        offstage: !showIcon,
        child: Icon(
          isSelected ? widget.selectedIcon : widget.unselectedIcon,
          size: iconSize,
          color: session
              ? (isSelected ? UiColor.text : UiColor.muted)
              : isSelected
                  ? MyTheme.tabbar(context).selectedTabIconColor
                  : MyTheme.tabbar(context).unSelectedTabIconColor,
        ).paddingOnly(right: session ? UiSession.tabIconGap : 5));
    final labelWidget = Obx(() {
      return ConstrainedBox(
          constraints: BoxConstraints(maxWidth: widget.maxLabelWidth ?? 200),
          child: Tooltip(
            message:
                widget.tabType == DesktopTabType.main ? '' : widget.label.value,
            child: Text(
              widget.tabType == DesktopTabType.main
                  ? translate(widget.label.value)
                  : widget.label.value,
              textAlign: TextAlign.center,
              style: session
                  ? UiType.sidebarItem.copyWith(
                      fontWeight:
                          isSelected ? FontWeight.w500 : FontWeight.w400,
                      color: isSelected ? UiColor.text : UiColor.muted)
                  : TextStyle(
                      color: isSelected
                          ? MyTheme.tabbar(context).selectedTextColor
                          : MyTheme.tabbar(context).unSelectedTextColor),
              overflow: TextOverflow.ellipsis,
            ),
          ));
    });

    Widget getWidgetWithBuilder() {
      if (widget.tabBuilder == null) {
        return Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            icon,
            labelWidget,
          ],
        );
      } else {
        return widget.tabBuilder!(
          widget.tabInfoKey,
          icon,
          labelWidget,
          TabThemeConf(iconSize: iconSize),
        );
      }
    }

    return Listener(
      onPointerDown: (e) {
        if (e.kind != ui.PointerDeviceKind.mouse) {
          return;
        }
        if (e.buttons == 2) {
          if (widget.tabMenuBuilder != null) {
            showRightMenu(
              (cacel) {
                return widget.tabMenuBuilder!(widget.tabInfoKey);
              },
              target: e.position,
            );
          }
        }
      },
      child: getWidgetWithBuilder(),
    );
  }

  @override
  Widget build(BuildContext context) {
    bool isSelected = widget.index == widget.selected;
    bool showDivider =
        widget.index != widget.selected - 1 && widget.index != widget.selected;
    final session = _isSessionTab(widget.tabType);
    RxBool hover = restoreHover.value.obs;
    return Ink(
      child: InkWell(
        onHover: (value) {
          hover.value = value;
          restoreHover.value = value;
        },
        onTap: () => widget.onTap(),
        child: Container(
            decoration: isSelected && widget.selectedBorderColor != null
                ? BoxDecoration(
                    border: Border(
                      bottom: BorderSide(
                        color: session
                            ? UiColor.primary
                            : widget.selectedBorderColor!,
                        width: session ? UiSession.tabIndicator : 1,
                      ),
                    ),
                  )
                : null,
            child: Container(
              color: session
                  ? null
                  : isSelected
                      ? widget.selectedTabBackgroundColor
                      : widget.unSelectedTabBackgroundColor,
              child: Row(
                children: [
                  SizedBox(
                      // _kTabBarHeight also displays normally
                      height: _showTabBarBottomDivider(widget.tabType)
                          ? _kTabBarHeight - 1
                          : _kTabBarHeight,
                      child: Row(
                          crossAxisAlignment: CrossAxisAlignment.center,
                          children: [
                            _buildTabContent(),
                            Obx((() => _CloseButton(
                                  visible: hover.value && widget.closable,
                                  tabSelected: isSelected,
                                  session: session,
                                  onClose: () => widget.onClose(),
                                )))
                          ])).paddingOnly(
                      left: session ? UiSession.tabPaddingX : 10,
                      right: session ? UiSpace.s2 : 5),
                  Offstage(
                    offstage: !showDivider,
                    child: VerticalDivider(
                      width: 1,
                      indent: _kDividerIndent,
                      endIndent: _kDividerIndent,
                      color: MyTheme.tabbar(context).dividerColor,
                    ),
                  )
                ],
              ),
            )),
      ),
    );
  }

  @override
  String? get restorationId => "_Tab${widget.label.value}";

  @override
  void restoreState(RestorationBucket? oldBucket, bool initialRestore) {
    registerForRestoration(restoreHover, 'restoreHover');
  }
}

class _CloseButton extends StatelessWidget {
  final bool visible;
  final bool tabSelected;
  final bool session;
  final Function onClose;

  const _CloseButton({
    Key? key,
    required this.visible,
    required this.tabSelected,
    this.session = false,
    required this.onClose,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    if (session) {
      return SizedBox(
          width: UiSession.tabCloseHitSize,
          height: UiSession.tabCloseHitSize,
          child: visible
              ? InkWell(
                  hoverColor: const Color(0x0f000000),
                  customBorder: const CircleBorder(),
                  onTap: () => onClose(),
                  child: Icon(Icons.close,
                      size: UiSession.tabCloseSize,
                      color:
                          tabSelected ? UiColor.textSecondary : UiColor.muted),
                )
              : null).paddingOnly(left: UiSpace.s1);
    }
    return SizedBox(
            width: _kIconSize,
            child: () {
              if (visible) {
                return InkWell(
                  hoverColor: MyTheme.tabbar(context).closeHoverColor,
                  customBorder: const CircleBorder(),
                  onTap: () => onClose(),
                  child: Icon(
                    Icons.close,
                    size: _kIconSize,
                    color: tabSelected
                        ? MyTheme.tabbar(context).selectedIconColor
                        : MyTheme.tabbar(context).unSelectedIconColor,
                  ),
                );
              } else {
                return Offstage();
              }
            }())
        .paddingOnly(left: 10);
  }
}
