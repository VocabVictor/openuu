import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:get/get.dart';

import '../../consts.dart';
import '../models/platform_model.dart';

import 'color_theme.dart';
import 'ffi_options.dart';
import 'globals.dart';
import 'windows_misc.dart';

class MyTheme {
  MyTheme._();

  static const Color grayBg = Color(0xFFEFEFF2);
  static const Color accent = Color(0xFF0071FF);
  static const Color accent50 = Color(0x770071FF);
  static const Color accent80 = Color(0xAA0071FF);
  static const Color canvasColor = Color(0xFF212121);
  static const Color border = Color(0xFFCCCCCC);
  static const Color idColor = Color(0xFF00B6F0);
  static const Color darkGray = Color.fromARGB(255, 148, 148, 148);
  static const Color cmIdColor = Color(0xFF21790B);
  static const Color dark = Colors.black87;
  static const Color button = Color(0xFF2C8CFF);
  static const Color hoverBorder = Color(0xFF999999);

  // ListTile
  static const ListTileThemeData listTileTheme = ListTileThemeData(
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.all(
        Radius.circular(5),
      ),
    ),
  );

  static SwitchThemeData switchTheme() {
    return SwitchThemeData(
        splashRadius: (isDesktop || isWebDesktop) ? 0 : kRadialReactionRadius);
  }

  static RadioThemeData radioTheme() {
    return RadioThemeData(
        splashRadius: (isDesktop || isWebDesktop) ? 0 : kRadialReactionRadius);
  }

  // Checkbox
  static const CheckboxThemeData checkboxTheme = CheckboxThemeData(
    splashRadius: 0,
    shape: RoundedRectangleBorder(
      borderRadius: BorderRadius.all(
        Radius.circular(5),
      ),
    ),
  );

  // TextButton
  // Value is used to calculate "dialog.actionsPadding"
  static const double mobileTextButtonPaddingLR = 20;

  // TextButton on mobile needs a fixed padding, otherwise small buttons
  // like "OK" has a larger left/right padding.
  static TextButtonThemeData mobileTextButtonTheme = TextButtonThemeData(
    style: TextButton.styleFrom(
      padding: EdgeInsets.symmetric(horizontal: mobileTextButtonPaddingLR),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8.0),
      ),
    ),
  );

  //tooltip
  static TooltipThemeData tooltipTheme() {
    return TooltipThemeData(
      waitDuration: Duration(seconds: 1, milliseconds: 500),
    );
  }

  // Dialogs
  static const double dialogPadding = 24;

  // padding bottom depends on content (some dialogs has no content)
  static EdgeInsets dialogTitlePadding({bool content = true}) {
    final double p = dialogPadding;

    return EdgeInsets.fromLTRB(p, p, p, content ? 0 : p);
  }

  // padding bottom depends on actions (mobile has dialogs without actions)
  static EdgeInsets dialogContentPadding({bool actions = true}) {
    final double p = dialogPadding;

    return (isDesktop || isWebDesktop)
        ? EdgeInsets.fromLTRB(p, p, p, actions ? (p - 4) : p)
        : EdgeInsets.fromLTRB(p, p, p, actions ? (p / 2) : p);
  }

  static EdgeInsets dialogActionsPadding() {
    final double p = dialogPadding;

    return (isDesktop || isWebDesktop)
        ? EdgeInsets.fromLTRB(p, 0, p, (p - 4))
        : EdgeInsets.fromLTRB(p, 0, (p - mobileTextButtonPaddingLR), (p / 2));
  }

  static EdgeInsets dialogButtonPadding = (isDesktop || isWebDesktop)
      ? EdgeInsets.only(left: dialogPadding)
      : EdgeInsets.only(left: dialogPadding / 3);

  static ScrollbarThemeData scrollbarTheme = ScrollbarThemeData(
    thickness: MaterialStateProperty.all(6),
    thumbColor: MaterialStateProperty.resolveWith<Color?>((states) {
      if (states.contains(MaterialState.dragged)) {
        return Colors.grey[900];
      } else if (states.contains(MaterialState.hovered)) {
        return Colors.grey[700];
      } else {
        return Colors.grey[500];
      }
    }),
    crossAxisMargin: 4,
  );

  static ScrollbarThemeData scrollbarThemeDark = scrollbarTheme.copyWith(
    thumbColor: MaterialStateProperty.resolveWith<Color?>((states) {
      if (states.contains(MaterialState.dragged)) {
        return Colors.grey[100];
      } else if (states.contains(MaterialState.hovered)) {
        return Colors.grey[300];
      } else {
        return Colors.grey[500];
      }
    }),
  );

  static ThemeData lightTheme = ThemeData(
    // https://stackoverflow.com/questions/77537315/after-upgrading-to-flutter-3-16-the-app-bar-background-color-button-size-and
    useMaterial3: false,
    brightness: Brightness.light,
    hoverColor: Color.fromARGB(255, 224, 224, 224),
    scaffoldBackgroundColor: Colors.white,
    dialogBackgroundColor: Colors.white,
    appBarTheme: AppBarTheme(
      shadowColor: Colors.transparent,
    ),
    dialogTheme: DialogTheme(
      elevation: 15,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(18.0),
        side: BorderSide(
          width: 1,
          color: grayBg,
        ),
      ),
    ),
    scrollbarTheme: scrollbarTheme,
    inputDecorationTheme: isDesktop
        ? InputDecorationTheme(
            fillColor: grayBg,
            filled: true,
            isDense: true,
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
            ),
          )
        : null,
    textTheme: const TextTheme(
        titleLarge: TextStyle(fontSize: 19, color: Colors.black87),
        titleSmall: TextStyle(fontSize: 14, color: Colors.black87),
        bodySmall: TextStyle(fontSize: 12, color: Colors.black87, height: 1.25),
        bodyMedium:
            TextStyle(fontSize: 14, color: Colors.black87, height: 1.25),
        labelLarge: TextStyle(fontSize: 16.0, color: MyTheme.accent80)),
    cardColor: grayBg,
    hintColor: Color(0xFFAAAAAA),
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: const TabBarTheme(
      labelColor: Colors.black87,
    ),
    tooltipTheme: tooltipTheme(),
    splashColor: (isDesktop || isWebDesktop) ? Colors.transparent : null,
    highlightColor: (isDesktop || isWebDesktop) ? Colors.transparent : null,
    splashFactory: (isDesktop || isWebDesktop) ? NoSplash.splashFactory : null,
    textButtonTheme: (isDesktop || isWebDesktop)
        ? TextButtonThemeData(
            style: TextButton.styleFrom(
              splashFactory: NoSplash.splashFactory,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(18.0),
              ),
            ),
          )
        : mobileTextButtonTheme,
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: MyTheme.accent,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8.0),
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        backgroundColor: grayBg,
        foregroundColor: Colors.black87,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8.0),
        ),
      ),
    ),
    switchTheme: switchTheme(),
    radioTheme: radioTheme(),
    checkboxTheme: checkboxTheme,
    listTileTheme: listTileTheme,
    menuBarTheme: MenuBarThemeData(
        style:
            MenuStyle(backgroundColor: MaterialStatePropertyAll(Colors.white))),
    colorScheme: ColorScheme.light(
        primary: Colors.blue, secondary: accent, background: grayBg),
    popupMenuTheme: PopupMenuThemeData(
        color: Colors.white,
        shape: RoundedRectangleBorder(
          side: BorderSide(
              color: (isDesktop || isWebDesktop)
                  ? Color(0xFFECECEC)
                  : Colors.transparent),
          borderRadius: BorderRadius.all(Radius.circular(8.0)),
        )),
  ).copyWith(
    extensions: <ThemeExtension<dynamic>>[
      ColorThemeExtension.light,
      TabbarTheme.light,
    ],
  );
  static ThemeData darkTheme = ThemeData(
    useMaterial3: false,
    brightness: Brightness.dark,
    hoverColor: Color.fromARGB(255, 45, 46, 53),
    scaffoldBackgroundColor: Color(0xFF18191E),
    dialogBackgroundColor: Color(0xFF18191E),
    appBarTheme: AppBarTheme(
      shadowColor: Colors.transparent,
    ),
    dialogTheme: DialogTheme(
      elevation: 15,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(18.0),
        side: BorderSide(
          width: 1,
          color: Color(0xFF24252B),
        ),
      ),
    ),
    scrollbarTheme: scrollbarThemeDark,
    inputDecorationTheme: (isDesktop || isWebDesktop)
        ? InputDecorationTheme(
            fillColor: Color(0xFF24252B),
            filled: true,
            isDense: true,
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(8),
            ),
          )
        : null,
    textTheme: const TextTheme(
      titleLarge: TextStyle(fontSize: 19),
      titleSmall: TextStyle(fontSize: 14),
      bodySmall: TextStyle(fontSize: 12, height: 1.25),
      bodyMedium: TextStyle(fontSize: 14, height: 1.25),
      labelLarge: TextStyle(
        fontSize: 16.0,
        fontWeight: FontWeight.bold,
        color: accent80,
      ),
    ),
    cardColor: Color(0xFF24252B),
    visualDensity: VisualDensity.adaptivePlatformDensity,
    tabBarTheme: const TabBarTheme(
      labelColor: Colors.white70,
    ),
    tooltipTheme: tooltipTheme(),
    splashColor: (isDesktop || isWebDesktop) ? Colors.transparent : null,
    highlightColor: (isDesktop || isWebDesktop) ? Colors.transparent : null,
    splashFactory: (isDesktop || isWebDesktop) ? NoSplash.splashFactory : null,
    textButtonTheme: (isDesktop || isWebDesktop)
        ? TextButtonThemeData(
            style: TextButton.styleFrom(
              splashFactory: NoSplash.splashFactory,
              disabledForegroundColor: Colors.white70,
              foregroundColor: Colors.white70,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(18.0),
              ),
            ),
          )
        : mobileTextButtonTheme,
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: MyTheme.accent,
        foregroundColor: Colors.white,
        disabledForegroundColor: Colors.white70,
        disabledBackgroundColor: Colors.white10,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8.0),
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        backgroundColor: Color(0xFF24252B),
        side: BorderSide(color: Colors.white12, width: 0.5),
        disabledForegroundColor: Colors.white70,
        foregroundColor: Colors.white70,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8.0),
        ),
      ),
    ),
    switchTheme: switchTheme(),
    radioTheme: radioTheme(),
    checkboxTheme: checkboxTheme,
    listTileTheme: listTileTheme,
    menuBarTheme: MenuBarThemeData(
        style: MenuStyle(
            backgroundColor: MaterialStatePropertyAll(Color(0xFF121212)))),
    colorScheme: ColorScheme.dark(
      primary: Colors.blue,
      secondary: accent,
      background: Color(0xFF24252B),
    ),
    popupMenuTheme: PopupMenuThemeData(
        shape: RoundedRectangleBorder(
      side: BorderSide(color: Colors.white24),
      borderRadius: BorderRadius.all(Radius.circular(8.0)),
    )),
  ).copyWith(
    extensions: <ThemeExtension<dynamic>>[
      ColorThemeExtension.dark,
      TabbarTheme.dark,
    ],
  );

  static ThemeMode getThemeModePreference() {
    return themeModeFromString(bind.mainGetLocalOption(key: kCommConfKeyTheme));
  }

  static Future<void> changeDarkMode(ThemeMode mode) async {
    Get.changeThemeMode(mode);
    if (desktopType == DesktopType.main || isAndroid || isIOS || isWeb) {
      if (mode == ThemeMode.system) {
        await bind.mainSetLocalOption(
            key: kCommConfKeyTheme, value: defaultOptionTheme);
      } else {
        await bind.mainSetLocalOption(
            key: kCommConfKeyTheme, value: mode.toShortString());
      }
      if (!isWeb) await bind.mainChangeTheme(dark: mode.toShortString());
      // Synchronize the window theme of the system.
      updateSystemWindowTheme();
    }
  }

  static ThemeMode currentThemeMode() {
    final preference = getThemeModePreference();
    if (preference == ThemeMode.system) {
      if (WidgetsBinding.instance.platformDispatcher.platformBrightness ==
          Brightness.light) {
        return ThemeMode.light;
      } else {
        return ThemeMode.dark;
      }
    } else {
      return preference;
    }
  }

  static ColorThemeExtension color(BuildContext context) {
    return Theme.of(context).extension<ColorThemeExtension>()!;
  }

  static TabbarTheme tabbar(BuildContext context) {
    return Theme.of(context).extension<TabbarTheme>()!;
  }

  static ThemeMode themeModeFromString(String v) {
    switch (v) {
      case "light":
        return ThemeMode.light;
      case "dark":
        return ThemeMode.dark;
      default:
        return ThemeMode.system;
    }
  }
}
