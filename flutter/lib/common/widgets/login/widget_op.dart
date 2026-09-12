part of 'login.dart';

class WidgetOP extends StatefulWidget {
  final ConfigOP config;
  final RxString curOP;
  final Function(Map<String, dynamic>) cbLogin;
  final Future<bool> Function(String) startAuth;
  final Future<bool> Function(String) cancelAuth;
  final bool Function() canStartAuth;
  const WidgetOP({
    Key? key,
    required this.config,
    required this.curOP,
    required this.cbLogin,
    required this.startAuth,
    required this.cancelAuth,
    required this.canStartAuth,
  }) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _WidgetOPState();
  }
}

class _WidgetOPState extends State<WidgetOP> {
  void _setState(VoidCallback fn) => setState(fn);
  Timer? _updateTimer;
  bool _isAuthStatusQueryInFlight = false;
  int _authAttempt = 0;
  String _stateMsg = '';
  String _failedMsg = '';
  String _url = '';

  @override
  void dispose() {
    super.dispose();
    _updateTimer?.cancel();
  }

  _beginQueryState(int authAttempt) {
    _updateTimer?.cancel();
    unawaited(_runAuthStatusQuery(() => _updateState(authAttempt)));
    _updateTimer = Timer.periodic(Duration(seconds: 1), (timer) {
      unawaited(_runAuthStatusQuery(() => _updateState(authAttempt)));
    });
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        ButtonOP(
          op: widget.config.op,
          curOP: widget.curOP,
          icon: widget.config.icon,
          primaryColor: str2color(widget.config.op, 0x7f),
          height: 36,
          canStartAuth: widget.canStartAuth,
          onTap: () async {
            if (!widget.canStartAuth()) {
              return;
            }
            final authAttempt = _resetState();
            try {
              final started = await widget.startAuth(widget.config.op);
              if (!started) {
                return;
              }
            } catch (e) {
              await _handleAuthFailure(
                authAttempt,
                e,
                'start account authentication',
              );
              return;
            }
            if (!mounted ||
                authAttempt != _authAttempt ||
                widget.curOP.value != widget.config.op) {
              return;
            }
            _beginQueryState(authAttempt);
          },
        ),
        Obx(() {
          if (widget.curOP.isNotEmpty &&
              widget.curOP.value != widget.config.op) {
            _failedMsg = '';
          }
          final authAttempt = _authAttempt;
          final authUrl = _url;
          return Offstage(
            offstage:
                _failedMsg.isEmpty && widget.curOP.value != widget.config.op,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                if (_stateMsg.isNotEmpty && _failedMsg.isEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 8.0),
                    child: OidcAuthStatus(
                      message: translate(_stateMsg),
                      browserFallbackPrompt: translate(
                        "Browser didn't open? Use the url below to sign in.",
                      ),
                      authUrl: authUrl,
                      copyLabel: translate('Copy to clipboard'),
                      onCopy: authUrl.isEmpty
                          ? null
                          : () => _runCurrentAuthUrlAction(
                                authAttempt,
                                authUrl,
                                _copyAuthUrl,
                              ),
                    ),
                  ),
                if (_failedMsg.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 8.0),
                    child: Builder(builder: (context) {
                      final errorColor = Theme.of(context).colorScheme.error;
                      final bgColor = Theme.of(context)
                          .colorScheme
                          .errorContainer
                          .withOpacity(0.3);
                      return Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 8.0, vertical: 6.0),
                        decoration: BoxDecoration(
                          color: bgColor,
                          borderRadius: BorderRadius.circular(4.0),
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(Icons.error_outline,
                                color: errorColor, size: 16),
                            const SizedBox(width: 6),
                            Flexible(
                              child: SelectableText(
                                translate(_failedMsg),
                                style:
                                    DefaultTextStyle.of(context).style.copyWith(
                                          fontSize: 13,
                                          color: errorColor,
                                        ),
                              ),
                            ),
                          ],
                        ),
                      );
                    }),
                  ),
              ],
            ),
          );
        }),
      ],
    );
  }
}
