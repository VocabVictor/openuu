part of 'popup_menu.dart';

class MenuEntrySubMenu<T> extends MenuEntryBase<T> {
  final String text;
  final List<MenuEntryBase<T>> entries;
  final EdgeInsets? padding;

  MenuEntrySubMenu({
    required this.text,
    required this.entries,
    this.padding,
    RxBool? enabled,
  }) : super(enabled: enabled);

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    super.enabled ??= true.obs;
    return [
      PopupMenuChildrenItem(
        enabled: super.enabled,
        height: conf.height,
        padding: padding,
        position: mod_menu.PopupMenuPosition.overSide,
        itemBuilder: (BuildContext context) => entries
            .map((entry) => entry.build(context, conf))
            .expand((i) => i)
            .toList(),
        child: Row(children: [
          const SizedBox(width: MenuConfig.midPadding),
          Obx(() => Text(
                text,
                style: super.enabled!.value
                    ? enabledStyle(context)
                    : disabledStyle(),
              )),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Obx(() => Icon(
                  Icons.keyboard_arrow_right,
                  color: super.enabled!.value ? conf.commonColor : Colors.grey,
                )),
          ))
        ]),
      )
    ];
  }
}

class MenuEntryButton<T> extends MenuEntryBase<T> {
  final Widget Function(TextStyle? style) childBuilder;
  Function() proc;
  final EdgeInsets? padding;

  MenuEntryButton({
    required this.childBuilder,
    required this.proc,
    this.padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        );

  Widget _buildChild(BuildContext context, MenuConfig conf) {
    super.enabled ??= true.obs;
    return Obx(() => Container(
        width: conf.boxWidth,
        child: TextButton(
          onPressed: super.enabled!.value
              ? () {
                  if (super.dismissOnClicked && Navigator.canPop(context)) {
                    Navigator.pop(context);
                    if (super.dismissCallback != null) {
                      super.dismissCallback!();
                    }
                  }
                  proc();
                }
              : null,
          child: Container(
            padding: padding,
            alignment: AlignmentDirectional.centerStart,
            constraints:
                BoxConstraints(minHeight: conf.height, maxHeight: conf.height),
            child: childBuilder(
                super.enabled!.value ? enabledStyle(context) : disabledStyle()),
          ),
        )));
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      mod_menu.PopupMenuItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        child: _buildChild(context, conf),
      )
    ];
  }
}

class CustomPopupMenuTheme {
  static const Color commonColor = MyTheme.accent;
  // kMinInteractiveDimension
  static const double height = 20.0;
  static const double dividerHeight = 3.0;
}
