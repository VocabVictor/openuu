part of 'login.dart';

class _OidcProviderBranding {
  final String label;
  final String iconKey;

  const _OidcProviderBranding({
    required this.label,
    required this.iconKey,
  });
}

_OidcProviderBranding _oidcProviderBranding(String op) {
  switch (op.toLowerCase()) {
    case 'azure':
      return _OidcProviderBranding(
        label: 'Microsoft',
        iconKey: 'microsoft',
      );
    default:
      return _OidcProviderBranding(
        label: {
              'github': 'GitHub',
              'gitlab': 'GitLab',
            }[op.toLowerCase()] ??
            toCapitalized(op),
        iconKey: op.toLowerCase(),
      );
  }
}

class _IconOP extends StatelessWidget {
  final String op;
  final String? icon;
  final EdgeInsets margin;
  const _IconOP(
      {Key? key,
      required this.op,
      required this.icon,
      this.margin = const EdgeInsets.symmetric(horizontal: 4.0)})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    final svgFile =
        kOpSvgList.contains(op.toLowerCase()) ? op.toLowerCase() : 'default';
    return Container(
      margin: margin,
      child: icon == null
          ? SvgPicture.asset(
              'assets/auth-$svgFile.svg',
              width: 20,
            )
          : SvgPicture.string(
              icon!,
              width: 20,
            ),
    );
  }
}

class ButtonOP extends StatelessWidget {
  final String op;
  final RxString curOP;
  final String? icon;
  final Color primaryColor;
  final double height;
  final Function() onTap;
  final bool Function() canStartAuth;

  const ButtonOP({
    Key? key,
    required this.op,
    required this.curOP,
    required this.icon,
    required this.primaryColor,
    required this.height,
    required this.onTap,
    required this.canStartAuth,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final branding = _oidcProviderBranding(op);
    final buttonLabel = translate("Continue with {${branding.label}}");
    return Row(children: [
      Container(
        height: isWindows ? 42 : height,
        width: isWindows ? 320 : 200,
        child: Obx(() => ElevatedButton(
            style: ElevatedButton.styleFrom(
              backgroundColor: isWindows ? Colors.white : primaryColor,
              foregroundColor: isWindows ? const Color(0xff303743) : null,
              side:
                  isWindows ? const BorderSide(color: Color(0xffdce2e9)) : null,
              shape: isWindows
                  ? RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8))
                  : null,
            ).copyWith(elevation: ButtonStyleButton.allOrNull(0.0)),
            onPressed:
                curOP.value == 'rustdesk' || !canStartAuth() ? null : onTap,
            child: Row(
              children: [
                SizedBox(
                  width: 30,
                  child: _IconOP(
                    op: branding.iconKey,
                    icon: icon,
                    margin: EdgeInsets.only(right: 5),
                  ),
                ),
                Expanded(
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: Center(child: Text(buttonLabel)),
                  ),
                ),
              ],
            ))),
      ),
    ]);
  }
}

class ConfigOP {
  final String op;
  final String? icon;
  ConfigOP({required this.op, required this.icon});
}

class _OidcAuthController {
  final RxString curOP = ''.obs;
  Future<void> _pendingOperation = Future<void>.value();
  int _authAttempt = 0;
  bool _closed = false;
  final _cancelInProgress = false.obs;

  bool _isCurrent(int authAttempt, String op) {
    return !_closed && authAttempt == _authAttempt && curOP.value == op;
  }

  Future<bool> start(String op) {
    if (!canStart()) {
      return Future<bool>.value(false);
    }
    final authAttempt = ++_authAttempt;
    curOP.value = op;
    // Web auth must start during the original user gesture so popups are allowed.
    final completer = Completer<bool>();
    _pendingOperation = _pendingOperation.then((_) async {
      if (!_isCurrent(authAttempt, op)) {
        completer.complete(false);
        return;
      }
      try {
        await bind.mainAccountAuthCancel();
        if (!_isCurrent(authAttempt, op)) {
          completer.complete(false);
          return;
        }
        await bind.mainAccountAuth(op: op, rememberMe: true);
        completer.complete(_isCurrent(authAttempt, op));
      } catch (error, stackTrace) {
        completer.completeError(error, stackTrace);
      }
    });
    return completer.future;
  }

  bool canStart() {
    return !_closed && !_cancelInProgress.value;
  }

  Future<bool> cancelCurrent(String op) {
    if (!canStart() || curOP.value != op) {
      return Future<bool>.value(false);
    }
    final authAttempt = ++_authAttempt;
    final completer = Completer<bool>();
    _cancelInProgress.value = true;
    _pendingOperation = _pendingOperation.then((_) async {
      try {
        await bind.mainAccountAuthCancel();
        completer.complete(_isCurrent(authAttempt, op));
      } catch (error, stackTrace) {
        completer.completeError(error, stackTrace);
      } finally {
        _cancelInProgress.value = false;
      }
    });
    return completer.future;
  }

  Future<void> _cancelBackend() async {
    try {
      await bind.mainAccountAuthCancel();
    } catch (error, stackTrace) {
      debugPrint('Failed to cancel account authentication $error');
      debugPrintStack(stackTrace: stackTrace);
    }
  }

  Future<void> close() async {
    if (_closed) {
      return;
    }
    final hasActiveOidcAuth =
        curOP.value.isNotEmpty && curOP.value != 'rustdesk';
    _closed = true;
    _authAttempt++;
    curOP.value = '';
    if (hasActiveOidcAuth) {
      await _cancelBackend();
    }
    await _pendingOperation;
    if (hasActiveOidcAuth) {
      await _cancelBackend();
    }
  }
}
