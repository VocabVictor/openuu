part of 'remote_toolbar.dart';

/// What a toolbar button is currently saying. It decides the button's
/// background and its icon colour together, so the two can never disagree.
///
/// The colours used to be read back from the background: the icon colour was
/// chosen by comparing the background against the tints. That works only while
/// no two states share a tint, and the day two of them do it picks the wrong
/// colour silently -- there is nothing to report, because a colour that
/// matched is indistinguishable from the colour that was meant.
enum _ButtonState {
  /// Nothing is claimed; the button is simply available.
  idle,

  /// The thing this button controls is on: pinned, recording, in a call,
  /// mobile actions showing, the display being viewed.
  engaged,

  /// Something is live that the user should not lose track of: a voice call
  /// waiting to be answered, a session being recorded. Pressing the button
  /// cancels that thing, which is the safe direction.
  ///
  /// Not `engaged`, which is a convenience the user chose to leave on and is
  /// not asking to be noticed. Not `destructive`, which is about what pressing
  /// costs rather than about what is currently true. It shares the danger tint
  /// with `destructive` today; keeping them apart is what lets one of them be
  /// given its own colour later without hunting through call sites.
  alerting,

  /// Pressing it ends the session. Kept apart from `alerting` because this one
  /// is about the cost of the press, not about a state to notice.
  destructive,
}

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
  // Button backgrounds. The bar is a surface-coloured outlined strip; a
  // button only paints a background on hover, when it is in an active state
  // (pinned, recording, call, mobile actions) or when it is the close/danger
  // button. Names are kept so the menu items read as before. Everything but
  // the transparent default resolves from the palette, so the dark theme is
  // one lookup away rather than a second set of literals.
  static const Color blueColor = Colors.transparent;
  static Color hoverBlueColor(BuildContext context) =>
      UiColor.of(context).settingsRowHover;
  static Color activeColor(BuildContext context) =>
      UiColor.of(context).primaryTint;
  static Color hoverActiveColor(BuildContext context) =>
      UiColor.of(context).primaryTintHover;

  // The three close affordances of a session window deliberately hover
  // differently, by how much the click costs: a tab close drops one tab and
  // uses a plain 6% black, while closing from the toolbar or the window ends
  // the session and earns this danger tint. Do not "unify" them.
  static Color redColor(BuildContext context) => UiColor.of(context).dangerTint;
  static Color hoverRedColor(BuildContext context) =>
      UiColor.of(context).dangerTintHover;
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
  /// the red tint, secondary text otherwise. Recovering the meaning from the
  /// colour is a reverse lookup and should become an explicit enum; comparing
  /// against the resolved palette at least makes it hold in both themes.
  /// The icon colour a state calls for.
  static Color iconColor(BuildContext context, _ButtonState state) {
    final pal = UiColor.of(context);
    switch (state) {
      case _ButtonState.engaged:
        return pal.primary;
      case _ButtonState.alerting:
      case _ButtonState.destructive:
        return pal.danger;
      case _ButtonState.idle:
        return pal.textSecondary;
    }
  }

  /// The background a state calls for, at rest and under the pointer. Taken
  /// from the same state as the icon so that a button cannot be painted as one
  /// thing and lettered as another.
  static Color buttonBackground(BuildContext context, _ButtonState state,
      {required bool hover}) {
    switch (state) {
      case _ButtonState.engaged:
        return hover ? hoverActiveColor(context) : activeColor(context);
      case _ButtonState.alerting:
      case _ButtonState.destructive:
        return hover ? hoverRedColor(context) : redColor(context);
      case _ButtonState.idle:
        return hover ? hoverBlueColor(context) : blueColor;
    }
  }

  static Color barColor(BuildContext context) => UiColor.of(context).surface;

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

  // Menus follow the design-review menu token: 8 radius, 1px border, a soft
  // shadow, 4 vertical padding, 32-high items with 12 side padding.
  static const double menuBorderRadius = UiSpace.menuRadius;
  static const EdgeInsets menuPadding =
      EdgeInsets.symmetric(vertical: UiSpace.menuPaddingY, horizontal: 4);
  static const double menuButtonBorderRadius = UiSpace.buttonRadius;
  static const double menuElevation = 4;

  static Color borderColor(BuildContext context) =>
      UiColor.of(context).border;

  static Color? dividerColor(BuildContext context) =>
      UiColor.of(context).settingsDivider;

  static MenuStyle defaultMenuStyle(BuildContext context) => MenuStyle(
        backgroundColor: WidgetStatePropertyAll(barColor(context)),
        surfaceTintColor: const WidgetStatePropertyAll(Colors.transparent),
        elevation: const WidgetStatePropertyAll(menuElevation),
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

  /// [enabled] false when the peer cannot accept the SAS: the entry is still
  /// listed, greyed, and says why, because leaving it out reads as "no such
  /// feature" (AGENTS.md).
  static insertCtrlAltDel(
    SessionID sessionId,
    EdgeInsets? padding, {
    DismissFunc? dismissFunc,
    DismissCallback? dismissCallback,
    bool enabled = true,
  }) {
    return MenuEntryButton<String>(
      enabled: enabled.obs,
      childBuilder: (TextStyle? style) {
        final label = Text(
          translate("Insert Ctrl + Alt + Del"),
          style: style,
        );
        return enabled
            ? label
            : Tooltip(
                message: translate('ctrl-alt-del-unavailable-tip'),
                child: label,
              );
      },
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
