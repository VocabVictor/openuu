part of 'popup_menu.dart';

class MenuEntryRadioOption {
  String text;
  String value;
  bool dismissOnClicked;
  RxBool? enabled;
  DismissCallback? dismissCallback;

  MenuEntryRadioOption({
    required this.text,
    required this.value,
    this.dismissOnClicked = false,
    this.enabled,
    this.dismissCallback,
  });
}

typedef RadioOptionsGetter = List<MenuEntryRadioOption> Function();

typedef RadioCurOptionGetter = Future<String> Function();

typedef RadioOptionSetter = Future<void> Function(
    String oldValue, String newValue);

class MenuEntryRadioUtils<T> {}

class MenuEntryRadios<T> extends MenuEntryBase<T> {
  final String text;
  final RadioOptionsGetter optionsGetter;
  final RadioCurOptionGetter curOptionGetter;
  final RadioOptionSetter optionSetter;
  final RxString _curOption = "".obs;
  final EdgeInsets? padding;

  MenuEntryRadios({
    required this.text,
    required this.optionsGetter,
    required this.curOptionGetter,
    required this.optionSetter,
    this.padding,
    dismissOnClicked = false,
    dismissCallback,
    RxBool? enabled,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        ) {
    () async {
      _curOption.value = await curOptionGetter();
    }();
  }

  List<MenuEntryRadioOption> get options => optionsGetter();
  RxString get curOption => _curOption;
  setOption(String option) async {
    await optionSetter(_curOption.value, option);
    if (_curOption.value != option) {
      final opt = await curOptionGetter();
      if (_curOption.value != opt) {
        _curOption.value = opt;
      }
    }
  }

  mod_menu.PopupMenuEntry<T> _buildMenuItem(
      BuildContext context, MenuConfig conf, MenuEntryRadioOption opt) {
    Widget getTextChild() {
      final enabledTextChild = Text(
        opt.text,
        style: enabledStyle(context),
      );
      final disabledTextChild = Text(
        opt.text,
        style: disabledStyle(),
      );
      if (opt.enabled == null) {
        return enabledTextChild;
      } else {
        return Obx(
            () => opt.enabled!.isTrue ? enabledTextChild : disabledTextChild);
      }
    }

    final child = Container(
      padding: padding,
      alignment: AlignmentDirectional.centerStart,
      constraints:
          BoxConstraints(minHeight: conf.height, maxHeight: conf.height),
      child: Row(
        children: [
          getTextChild(),
          Expanded(
              child: Align(
                  alignment: Alignment.centerRight,
                  child: Transform.scale(
                    scale: MenuConfig.iconScale,
                    child: Obx(() => opt.value == curOption.value
                        ? IconButton(
                            padding:
                                const EdgeInsets.fromLTRB(8.0, 0.0, 8.0, 0.0),
                            hoverColor: Colors.transparent,
                            focusColor: Colors.transparent,
                            onPressed: () {},
                            icon: Icon(
                              Icons.check,
                              color: (opt.enabled ?? true.obs).isTrue
                                  ? conf.commonColor
                                  : Colors.grey,
                            ))
                        : const SizedBox.shrink()),
                  ))),
        ],
      ),
    );
    onPressed() {
      if (opt.dismissOnClicked && Navigator.canPop(context)) {
        Navigator.pop(context);
        if (opt.dismissCallback != null) {
          opt.dismissCallback!();
        }
      }
      setOption(opt.value);
    }

    return mod_menu.PopupMenuItem(
      padding: EdgeInsets.zero,
      height: conf.height,
      child: Container(
        width: conf.boxWidth,
        child: opt.enabled == null
            ? TextButton(
                child: child,
                onPressed: onPressed,
              )
            : Obx(() => TextButton(
                  child: child,
                  onPressed: opt.enabled!.isTrue ? onPressed : null,
                )),
      ),
    );
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return options.map((opt) => _buildMenuItem(context, conf, opt)).toList();
  }
}

class MenuEntrySubRadios<T> extends MenuEntryBase<T> {
  final String text;
  final RadioOptionsGetter optionsGetter;
  final RadioCurOptionGetter curOptionGetter;
  final RadioOptionSetter optionSetter;
  final RxString _curOption = "".obs;
  final EdgeInsets? padding;

  MenuEntrySubRadios({
    required this.text,
    required this.optionsGetter,
    required this.curOptionGetter,
    required this.optionSetter,
    this.padding,
    dismissOnClicked = false,
    RxBool? enabled,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
        ) {
    () async {
      _curOption.value = await curOptionGetter();
    }();
  }

  List<MenuEntryRadioOption> get options => optionsGetter();
  RxString get curOption => _curOption;
  setOption(String option) async {
    await optionSetter(_curOption.value, option);
    if (_curOption.value != option) {
      final opt = await curOptionGetter();
      if (_curOption.value != opt) {
        _curOption.value = opt;
      }
    }
  }

  mod_menu.PopupMenuEntry<T> _buildSecondMenu(
      BuildContext context, MenuConfig conf, MenuEntryRadioOption opt) {
    return mod_menu.PopupMenuItem(
      padding: EdgeInsets.zero,
      height: conf.height,
      child: Container(
          width: conf.boxWidth,
          child: TextButton(
            child: Container(
              padding: padding,
              alignment: AlignmentDirectional.centerStart,
              constraints: BoxConstraints(
                  minHeight: conf.height, maxHeight: conf.height),
              child: Row(
                children: [
                  Text(
                    opt.text,
                    style: TextStyle(
                        color: Theme.of(context).textTheme.titleLarge?.color,
                        fontSize: MenuConfig.fontSize,
                        fontWeight: FontWeight.normal),
                  ),
                  Expanded(
                      child: Align(
                    alignment: Alignment.centerRight,
                    child: Transform.scale(
                        scale: MenuConfig.iconScale,
                        child: Obx(() => opt.value == curOption.value
                            ? IconButton(
                                padding: EdgeInsets.zero,
                                hoverColor: Colors.transparent,
                                focusColor: Colors.transparent,
                                onPressed: () {},
                                icon: Icon(
                                  Icons.check,
                                  color: conf.commonColor,
                                ))
                            : const SizedBox.shrink())),
                  )),
                ],
              ),
            ),
            onPressed: () {
              if (opt.dismissOnClicked && Navigator.canPop(context)) {
                Navigator.pop(context);
                if (opt.dismissCallback != null) {
                  opt.dismissCallback!();
                }
              }
              setOption(opt.value);
            },
          )),
    );
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      PopupMenuChildrenItem(
        enabled: super.enabled,
        padding: padding,
        height: conf.height,
        itemBuilder: (BuildContext context) =>
            options.map((opt) => _buildSecondMenu(context, conf, opt)).toList(),
        child: Row(children: [
          const SizedBox(width: MenuConfig.midPadding),
          Text(
            text,
            style: TextStyle(
                color: Theme.of(context).textTheme.titleLarge?.color,
                fontSize: MenuConfig.fontSize,
                fontWeight: FontWeight.normal),
          ),
          Expanded(
              child: Align(
            alignment: Alignment.centerRight,
            child: Icon(
              Icons.keyboard_arrow_right,
              color: conf.commonColor,
            ),
          ))
        ]),
      )
    ];
  }
}
