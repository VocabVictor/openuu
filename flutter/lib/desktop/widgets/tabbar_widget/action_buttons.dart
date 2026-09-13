part of 'tabbar_widget.dart';

class ActionIcon extends StatefulWidget {
  final String? message;
  final IconData icon;
  final GestureTapCallback? onTap;
  final GestureTapDownCallback? onTapDown;
  final bool isClose;
  final double iconSize;
  final double boxSize;

  /// Session-window look: 14px grey glyph, row hover, close hover in a
  /// danger tint with a danger glyph instead of the red block.
  final bool session;

  const ActionIcon(
      {Key? key,
      this.message,
      required this.icon,
      this.onTap,
      this.onTapDown,
      this.isClose = false,
      this.iconSize = _kActionIconSize,
      this.boxSize = _kTabBarHeight - 1,
      this.session = false})
      : super(key: key);

  @override
  State<ActionIcon> createState() => _ActionIconState();
}

class _ActionIconState extends State<ActionIcon> {
  final hover = false.obs;

  @override
  Widget build(BuildContext context) {
    final pal = UiColor.of(context);
    final session = widget.session;
    final iconSize = session ? UiSession.tabActionIconSize : widget.iconSize;
    return Tooltip(
      message: widget.message != null ? translate(widget.message!) : "",
      waitDuration: const Duration(seconds: 1),
      child: InkWell(
        hoverColor: session
            ? (widget.isClose
                ? pal.danger.withOpacity(.12)
                : pal.settingsRowHover)
            : widget.isClose
                ? const Color.fromARGB(255, 196, 43, 28)
                : MyTheme.tabbar(context).hoverColor,
        onHover: (value) => hover.value = value,
        onTap: widget.onTap,
        onTapDown: widget.onTapDown,
        child: SizedBox(
          height: widget.boxSize,
          width: widget.boxSize,
          child: widget.onTap == null
              ? Icon(
                  widget.icon,
                  color: session ? pal.faint : Colors.grey,
                  size: iconSize,
                )
              : Obx(
                  () => Icon(
                    widget.icon,
                    color: session
                        ? (hover.value && widget.isClose
                            ? pal.danger
                            : pal.textSecondary)
                        : hover.value && widget.isClose
                            ? pal.onPrimary
                            : MyTheme.tabbar(context).unSelectedIconColor,
                    size: iconSize,
                  ),
                ),
        ),
      ),
    );
  }
}

class AddButton extends StatelessWidget {
  const AddButton({
    Key? key,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ActionIcon(
        message: 'New Connection',
        icon: IconFont.add,
        onTap: () => rustDeskWinManager.call(
            WindowType.Main, kWindowMainWindowOnTop, ""),
        isClose: false);
  }
}

class _TabDropDownButton extends StatefulWidget {
  final DesktopTabController controller;
  final List<String> tabkeys;
  final LabelGetter? labelGetter;

  const _TabDropDownButton(
      {required this.controller, required this.tabkeys, this.labelGetter});

  @override
  State<_TabDropDownButton> createState() => _TabDropDownButtonState();
}

class _TabDropDownButtonState extends State<_TabDropDownButton> {
  var position = RelativeRect.fromLTRB(0, 0, 0, 0);

  @override
  Widget build(BuildContext context) {
    List<String> sortedKeys = widget.controller.state.value.tabs
        .where((e) => widget.tabkeys.contains(e.key))
        .map((e) => e.key)
        .toList();
    return ActionIcon(
      onTapDown: (details) {
        final x = details.globalPosition.dx;
        final y = details.globalPosition.dy;
        position = RelativeRect.fromLTRB(x, y, x, y);
      },
      icon: Icons.arrow_drop_down,
      onTap: () {
        showMenu(
          context: context,
          position: position,
          items: sortedKeys.map((e) {
            var label = e;
            final tabInfo = widget.controller.state.value.tabs
                .firstWhereOrNull((element) => element.key == e);
            if (tabInfo != null) {
              label = tabInfo.label;
            }
            if (widget.labelGetter != null) {
              label = widget.labelGetter!(e).value;
            }
            var index = widget.controller.state.value.tabs
                .indexWhere((t) => t.key == e);
            label = '${index + 1}. $label';
            final menuHover = false.obs;
            final btnHover = false.obs;
            return PopupMenuItem<String>(
              value: e,
              height: 32,
              onTap: () {
                widget.controller.jumpToByKey(e);
                if (Navigator.of(context).canPop()) {
                  Navigator.of(context).pop();
                }
              },
              child: MouseRegion(
                onHover: (event) => setState(() => menuHover.value = true),
                onExit: (event) => setState(() => menuHover.value = false),
                child: Row(
                  children: [
                    Expanded(
                      child: InkWell(child: Text(label)),
                    ),
                    Obx(
                      () {
                        if (tabInfo?.onTabCloseButton != null &&
                            menuHover.value) {
                          return InkWell(
                              onTap: () {
                                tabInfo?.onTabCloseButton?.call();
                                if (Navigator.of(context).canPop()) {
                                  Navigator.of(context).pop();
                                }
                              },
                              child: MouseRegion(
                                  cursor: SystemMouseCursors.click,
                                  onHover: (event) =>
                                      setState(() => btnHover.value = true),
                                  onExit: (event) =>
                                      setState(() => btnHover.value = false),
                                  child: Icon(Icons.close,
                                      color:
                                          btnHover.value ? Colors.red : null)));
                        } else {
                          return Offstage();
                        }
                      },
                    ),
                  ],
                ),
              ),
            );
          }).toList(),
        );
      },
    );
  }
}
