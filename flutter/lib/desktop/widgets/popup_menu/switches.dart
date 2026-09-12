part of 'popup_menu.dart';

enum SwitchType {
  sswitch,
  scheckbox,
}

typedef SwitchGetter = Future<bool> Function();

typedef SwitchSetter = Future<void> Function(bool);

abstract class MenuEntrySwitchBase<T> extends MenuEntryBase<T> {
  final SwitchType switchType;
  final String text;
  final EdgeInsets? padding;
  Rx<TextStyle>? textStyle;

  MenuEntrySwitchBase({
    required this.switchType,
    required this.text,
    required dismissOnClicked,
    this.textStyle,
    this.padding,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        );

  bool get isEnabled => enabled?.value ?? true;

  RxBool get curOption;
  Future<void> setOption(bool? option);

  tryPop(BuildContext context) {
    if (dismissOnClicked && Navigator.canPop(context)) {
      Navigator.pop(context);
      super.dismissCallback?.call();
    }
  }

  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    textStyle ??= TextStyle(
            color: Theme.of(context).textTheme.titleLarge?.color,
            fontSize: MenuConfig.fontSize,
            fontWeight: FontWeight.normal)
        .obs;
    return [
      mod_menu.PopupMenuItem(
        padding: EdgeInsets.zero,
        height: conf.height,
        child: Container(
            width: conf.boxWidth,
            child: TextButton(
              child: Container(
                  padding: padding,
                  alignment: AlignmentDirectional.centerStart,
                  height: conf.height,
                  child: Row(children: [
                    Obx(() => Text(
                          text,
                          style: textStyle!.value,
                        )),
                    Expanded(
                        child: Align(
                      alignment: Alignment.centerRight,
                      child: Transform.scale(
                          scale: MenuConfig.iconScale,
                          child: Obx(() {
                            if (switchType == SwitchType.sswitch) {
                              return Switch(
                                value: curOption.value,
                                onChanged: isEnabled
                                    ? (v) {
                                        tryPop(context);
                                        setOption(v);
                                      }
                                    : null,
                              );
                            } else {
                              return Checkbox(
                                value: curOption.value,
                                onChanged: isEnabled
                                    ? (v) {
                                        tryPop(context);
                                        setOption(v);
                                      }
                                    : null,
                              );
                            }
                          })),
                    ))
                  ])),
              onPressed: isEnabled
                  ? () {
                      tryPop(context);
                      setOption(!curOption.value);
                    }
                  : null,
            )),
      )
    ];
  }
}

class MenuEntrySwitch<T> extends MenuEntrySwitchBase<T> {
  final SwitchGetter getter;
  final SwitchSetter setter;
  final RxBool _curOption = false.obs;

  MenuEntrySwitch({
    required SwitchType switchType,
    required String text,
    required this.getter,
    required this.setter,
    Rx<TextStyle>? textStyle,
    EdgeInsets? padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          switchType: switchType,
          text: text,
          textStyle: textStyle,
          padding: padding,
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        ) {
    () async {
      _curOption.value = await getter();
    }();
  }

  @override
  RxBool get curOption => _curOption;
  @override
  setOption(bool? option) async {
    if (option != null) {
      await setter(option);
      final opt = await getter();
      if (_curOption.value != opt) {
        _curOption.value = opt;
      }
    }
  }
}

// Compatible with MenuEntrySwitch, it uses value instead of getter
class MenuEntrySwitchSync<T> extends MenuEntrySwitchBase<T> {
  final SwitchSetter setter;
  final RxBool _curOption = false.obs;

  MenuEntrySwitchSync({
    required SwitchType switchType,
    required String text,
    required bool currentValue,
    required this.setter,
    Rx<TextStyle>? textStyle,
    EdgeInsets? padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          switchType: switchType,
          text: text,
          textStyle: textStyle,
          padding: padding,
          dismissOnClicked: dismissOnClicked,
          enabled: enabled,
          dismissCallback: dismissCallback,
        ) {
    _curOption.value = currentValue;
  }

  @override
  RxBool get curOption => _curOption;
  @override
  setOption(bool? option) async {
    if (option != null) {
      await setter(option);
      // Notice: no ensure with getter, best used on menus that are destroyed on click
      if (_curOption.value != option) {
        _curOption.value = option;
      }
    }
  }
}

typedef Switch2Getter = RxBool Function();

typedef Switch2Setter = Future<void> Function(bool);

class MenuEntrySwitch2<T> extends MenuEntrySwitchBase<T> {
  final Switch2Getter getter;
  final SwitchSetter setter;

  MenuEntrySwitch2({
    required SwitchType switchType,
    required String text,
    required this.getter,
    required this.setter,
    Rx<TextStyle>? textStyle,
    EdgeInsets? padding,
    dismissOnClicked = false,
    RxBool? enabled,
    dismissCallback,
  }) : super(
          switchType: switchType,
          text: text,
          textStyle: textStyle,
          padding: padding,
          dismissOnClicked: dismissOnClicked,
          dismissCallback: dismissCallback,
        );

  @override
  RxBool get curOption => getter();
  @override
  setOption(bool? option) async {
    if (option != null) {
      await setter(option);
    }
  }
}
