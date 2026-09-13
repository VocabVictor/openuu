part of 'remote_toolbar.dart';

class ToolbarState {
  late RxBool _pin;

  RxBool collapse = false.obs;
  RxBool hide = false.obs;

  // Track initialization state to prevent flickering
  final RxBool initialized = false.obs;
  bool _isInitializing = false;

  ToolbarState() {
    _pin = RxBool(false);
    final s = bind.getLocalFlutterOption(k: kOptionRemoteMenubarState);
    if (s.isEmpty) {
      return;
    }

    try {
      final m = jsonDecode(s);
      if (m != null) {
        _pin = RxBool(m['pin'] ?? false);
      }
    } catch (e) {
      debugPrint('Failed to decode toolbar state ${e.toString()}');
    }
  }

  bool get pin => _pin.value;

  /// Initialize all toolbar states from session options.
  /// This should be called once when the toolbar is first created.
  Future<void> init(SessionID sessionId) async {
    if (initialized.value || _isInitializing) return;
    _isInitializing = true;

    try {
      // Load both states in parallel for better performance
      final results = await Future.wait([
        bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionCollapseToolbar),
        bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionHideToolbar),
      ]);

      collapse.value = results[0] ?? false;
      hide.value = results[1] ?? false;
    } finally {
      _isInitializing = false;
      initialized.value = true;
    }
  }

  switchCollapse(SessionID sessionId) async {
    bind.sessionToggleOption(
        sessionId: sessionId, value: kOptionCollapseToolbar);
    collapse.value = !collapse.value;
  }

  // Switch hide state for entire toolbar visibility
  switchHide(SessionID sessionId) async {
    bind.sessionToggleOption(sessionId: sessionId, value: kOptionHideToolbar);
    hide.value = !hide.value;
  }

  switchPin() async {
    _pin.value = !_pin.value;
    // Save everytime changed, as this func will not be called frequently
    await _savePin();
  }

  setPin(bool v) async {
    if (_pin.value != v) {
      _pin.value = v;
      // Save everytime changed, as this func will not be called frequently
      await _savePin();
    }
  }

  _savePin() async {
    bind.setLocalFlutterOption(
        k: kOptionRemoteMenubarState, v: jsonEncode({'pin': _pin.value}));
  }
}

class _ToolbarTheme {
  // Button backgrounds. The bar is a white (dark: #22262c) outlined strip; a
  // button only paints a background on hover, when it is in an active state
  // (pinned, recording, call, mobile actions) or when it is the close/danger
  // button. Names are kept so the menu items read as before.
  static const Color blueColor = Colors.transparent;
  static const Color hoverBlueColor = UiColor.settingsRowHover;
  static Color inactiveColor = Colors.transparent;
  static Color hoverInactiveColor = UiColor.settingsRowHover;
  static const Color activeColor = UiColor.primaryTint;
  static const Color hoverActiveColor = Color(0xffdce8ff);

  static const Color redColor = Color(0xfffff0ef);
  static const Color hoverRedColor = Color(0xffffe1df);
  // kMinInteractiveDimension
  static const double height = 20.0;
  static const double dividerHeight = 12.0;

  static const double buttonSize = UiSession.toolbarButtonSize;
  static const double iconSize = UiSession.toolbarIconSize;
  static const double buttonHMargin = UiSession.toolbarButtonGap / 2;
  static const double buttonVMargin = 6;
  static const double iconRadius = UiSession.toolbarButtonRadius;
  static const double barRadius = UiSession.toolbarRadius;
  static const double elevation = 0;

  /// Icon tint for a button background: primary on the active tint, danger on
  /// the red tint, secondary text otherwise.
  static Color iconColor(Color background, {bool dark = false}) {
    if (background == activeColor || background == hoverActiveColor) {
      return UiColor.primary;
    }
    if (background == redColor || background == hoverRedColor) {
      return UiColor.danger;
    }
    return dark ? const Color(0xffc9cdd4) : UiColor.textSecondary;
  }

  static Color barColor(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark
          ? const Color(0xff22262c)
          : Colors.white;

  static BoxDecoration barDecoration(BuildContext context) => BoxDecoration(
        color: barColor(context),
        border: Border.all(color: borderColor(context), width: 1),
        borderRadius: BorderRadius.circular(barRadius),
        boxShadow: const [
          BoxShadow(
            color: UiSession.toolbarShadow,
            offset: UiSession.toolbarShadowOffset,
            blurRadius: UiSession.toolbarShadowBlur,
          ),
        ],
      );

  static double dividerSpaceToAction = isWindows ? 8 : 14;

  static double menuBorderRadius = isWindows ? 5.0 : 7.0;
  static EdgeInsets menuPadding = isWindows
      ? EdgeInsets.fromLTRB(4, 12, 4, 12)
      : EdgeInsets.fromLTRB(6, 14, 6, 14);
  static const double menuButtonBorderRadius = 3.0;

  static Color borderColor(BuildContext context) =>
      Theme.of(context).brightness == Brightness.dark
          ? (MyTheme.color(context).border3 ?? MyTheme.border)
          : UiColor.border;

  static Color? dividerColor(BuildContext context) =>
      MyTheme.color(context).divider;

  static MenuStyle defaultMenuStyle(BuildContext context) => MenuStyle(
        side: MaterialStateProperty.all(BorderSide(
          width: 1,
          color: borderColor(context),
        )),
        shape: MaterialStatePropertyAll(RoundedRectangleBorder(
            borderRadius:
                BorderRadius.circular(_ToolbarTheme.menuBorderRadius))),
        padding: MaterialStateProperty.all(_ToolbarTheme.menuPadding),
      );
  static final defaultMenuButtonStyle = ButtonStyle(
    backgroundColor: MaterialStatePropertyAll(Colors.transparent),
    padding: MaterialStatePropertyAll(EdgeInsets.zero),
    overlayColor: MaterialStatePropertyAll(Colors.transparent),
  );

  static Widget borderWrapper(
      BuildContext context, Widget child, BorderRadius borderRadius) {
    return Container(
      decoration: BoxDecoration(
        border: Border.all(
          color: borderColor(context),
          width: 1,
        ),
        borderRadius: borderRadius,
      ),
      child: child,
    );
  }
}

typedef DismissFunc = void Function();

class RemoteMenuEntry {
  static MenuEntryButton<String> insertLock(
    SessionID sessionId,
    EdgeInsets? padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate('Insert Lock'),
        style: style,
      ),
      proc: () {
        bind.sessionLockScreen(sessionId: sessionId);
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: dismissCallback,
    );
  }

  static insertCtrlAltDel(
    SessionID sessionId,
    EdgeInsets? padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
  }) {
    return MenuEntryButton<String>(
      childBuilder: (TextStyle? style) => Text(
        translate("Insert Ctrl + Alt + Del"),
        style: style,
      ),
      proc: () {
        bind.sessionCtrlAltDel(sessionId: sessionId);
        if (dismissFunc != null) {
          dismissFunc();
        }
      },
      padding: padding,
      dismissOnClicked: true,
      dismissCallback: dismissCallback,
    );
  }
}

class InputModeMenu {
  final String key;
  final String menu;

  InputModeMenu({required this.key, required this.menu});
}

_menuDismissCallback(FFI ffi) => ffi.inputModel.refreshMousePos();

Widget _buildPointerTrackWidget(Widget child, FFI? ffi) {
  return Listener(
    onPointerHover: (PointerHoverEvent e) => {
      if (ffi != null) {ffi.inputModel.lastMousePos = e.position}
    },
    child: MouseRegion(
      child: child,
    ),
  );
}
