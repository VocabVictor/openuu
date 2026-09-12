import 'dart:core';

import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../../common.dart';
import '.././material_mod_popup_menu.dart' as mod_menu;
part 'radios.dart';
part 'switches.dart';
part 'entries.dart';

// https://stackoverflow.com/questions/68318314/flutter-popup-menu-inside-popup-menu
class PopupMenuChildrenItem<T> extends mod_menu.PopupMenuEntry<T> {
  const PopupMenuChildrenItem({
    key,
    this.height = kMinInteractiveDimension,
    this.padding,
    this.enabled,
    this.textStyle,
    this.onTap,
    this.position = mod_menu.PopupMenuPosition.overSide,
    this.offset = Offset.zero,
    required this.itemBuilder,
    required this.child,
  }) : super(key: key);

  final mod_menu.PopupMenuPosition position;
  final Offset offset;
  final TextStyle? textStyle;
  final EdgeInsets? padding;
  final RxBool? enabled;
  final void Function()? onTap;
  final List<mod_menu.PopupMenuEntry<T>> Function(BuildContext) itemBuilder;
  final Widget child;

  @override
  final double height;

  @override
  bool represents(T? value) => false;

  @override
  MyPopupMenuItemState<T, PopupMenuChildrenItem<T>> createState() =>
      MyPopupMenuItemState<T, PopupMenuChildrenItem<T>>(enabled?.value);
}

class MyPopupMenuItemState<T, W extends PopupMenuChildrenItem<T>>
    extends State<W> {
  RxBool enabled = true.obs;

  MyPopupMenuItemState(bool? e) {
    if (e != null) {
      enabled.value = e;
    }
  }

  @protected
  void handleTap(T value) {
    widget.onTap?.call();
    Navigator.pop<T>(context, value);
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final PopupMenuThemeData popupMenuTheme = PopupMenuTheme.of(context);
    TextStyle style = widget.textStyle ??
        popupMenuTheme.textStyle ??
        theme.textTheme.titleMedium!;
    return Obx(() => mod_menu.PopupMenuButton<T>(
          enabled: enabled.value,
          position: widget.position,
          offset: widget.offset,
          onSelected: handleTap,
          itemBuilder: widget.itemBuilder,
          padding: EdgeInsets.zero,
          child: AnimatedDefaultTextStyle(
            style: style,
            duration: kThemeChangeDuration,
            child: Container(
              alignment: AlignmentDirectional.centerStart,
              constraints: BoxConstraints(
                  minHeight: widget.height, maxHeight: widget.height),
              padding:
                  widget.padding ?? const EdgeInsets.symmetric(horizontal: 16),
              child: widget.child,
            ),
          ),
        ));
  }
}

class MenuConfig {
  // adapt to the screen height
  static const fontSize = 14.0;
  static const midPadding = 10.0;
  static const iconScale = 0.8;
  static const iconWidth = 12.0;
  static const iconHeight = 12.0;

  final double height;
  final double dividerHeight;
  final double? boxWidth;
  final Color commonColor;

  const MenuConfig(
      {required this.commonColor,
      this.height = kMinInteractiveDimension,
      this.dividerHeight = 16.0,
      this.boxWidth});
}

typedef DismissCallback = Function();

abstract class MenuEntryBase<T> {
  bool dismissOnClicked;
  DismissCallback? dismissCallback;
  RxBool? enabled;

  MenuEntryBase({
    this.dismissOnClicked = false,
    this.enabled,
    this.dismissCallback,
  });
  List<mod_menu.PopupMenuEntry<T>> build(BuildContext context, MenuConfig conf);

  enabledStyle(BuildContext context) => TextStyle(
      color: Theme.of(context).textTheme.titleLarge?.color,
      fontSize: MenuConfig.fontSize,
      fontWeight: FontWeight.normal);
  disabledStyle() => TextStyle(
      color: Colors.grey,
      fontSize: MenuConfig.fontSize,
      fontWeight: FontWeight.normal);
}

class MenuEntryDivider<T> extends MenuEntryBase<T> {
  @override
  List<mod_menu.PopupMenuEntry<T>> build(
      BuildContext context, MenuConfig conf) {
    return [
      mod_menu.PopupMenuDivider(
        height: conf.dividerHeight,
      )
    ];
  }
}
