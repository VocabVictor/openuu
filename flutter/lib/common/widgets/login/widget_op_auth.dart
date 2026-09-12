part of 'login.dart';

extension _WidgetOPAuth on _WidgetOPState {
  Future<void> _runAuthStatusQuery(Future<void> Function() query) async {
    if (_isAuthStatusQueryInFlight) {
      return;
    }
    _isAuthStatusQueryInFlight = true;
    try {
      await query();
    } finally {
      _isAuthStatusQueryInFlight = false;
    }
  }

  Future<void> _launchAuthUrl(String url) async {
    try {
      final launched = await launchUrl(
        Uri.parse(url),
        mode: LaunchMode.externalApplication,
      );
      if (!launched) {
        debugPrint('Failed to open OIDC authentication URL');
      }
    } catch (error, stackTrace) {
      debugPrint(
          'Failed to open OIDC authentication URL (${error.runtimeType})');
      debugPrintStack(stackTrace: stackTrace);
    }
  }

  Future<void> _copyAuthUrl(String url) async {
    try {
      await Clipboard.setData(ClipboardData(text: url));
      showToast(
        translate('Copied'),
      );
    } catch (error, stackTrace) {
      debugPrint(
          'Failed to copy OIDC authentication URL (${error.runtimeType})');
      debugPrintStack(stackTrace: stackTrace);
      showToast(translate('Failed'));
    }
  }

  void _runCurrentAuthUrlAction(
    int authAttempt,
    String authUrl,
    Future<void> Function(String) action,
  ) {
    if (!mounted ||
        authAttempt != _authAttempt ||
        widget.curOP.value != widget.config.op ||
        authUrl.isEmpty ||
        _url != authUrl) {
      return;
    }
    unawaited(action(authUrl));
  }

  void _invalidateAuthAttempt() {
    _authAttempt++;
    _url = '';
  }

  bool _isCurrentAuthAttempt(int authAttempt) {
    return mounted &&
        authAttempt == _authAttempt &&
        widget.curOP.value == widget.config.op;
  }

  Future<void> _handleAuthFailure(
    int authAttempt,
    Object error,
    String operation,
  ) async {
    debugPrint('Failed to $operation $error');
    if (!_isCurrentAuthAttempt(authAttempt)) {
      return;
    }
    _updateTimer?.cancel();
    _setState(() => _failedMsg = 'Failed');
    try {
      final canceled = await widget.cancelAuth(widget.config.op);
      if (!canceled || !_isCurrentAuthAttempt(authAttempt)) {
        return;
      }
    } catch (cancelError, stackTrace) {
      debugPrint('Failed to cancel account authentication $cancelError');
      debugPrintStack(stackTrace: stackTrace);
      return;
    }
    _setState(() {
      _invalidateAuthAttempt();
      widget.curOP.value = '';
    });
  }

  Future<void> _updateState(int authAttempt) {
    if (!mounted ||
        authAttempt != _authAttempt ||
        widget.curOP.value != widget.config.op) {
      _updateTimer?.cancel();
      return Future<void>.value();
    }
    return bind.mainAccountAuthResult().then<void>((result) {
      if (!mounted ||
          authAttempt != _authAttempt ||
          widget.curOP.value != widget.config.op ||
          result.isEmpty) {
        return;
      }
      final resultMap = jsonDecode(result);
      if (resultMap == null) {
        return;
      }
      final String backendStateMsg = resultMap['state_msg'];
      String failedMsg = resultMap['failed_msg'];
      final String? url = resultMap['url'];
      final stateMsg = backendStateMsg == _requestingAccountAuth &&
              (url == null || url.isEmpty)
          ? _waitingAccountAuth
          : backendStateMsg;
      final bool urlLaunched = (resultMap['url_launched'] as bool?) ?? false;
      final authBody = resultMap['auth_body'];
      if (authBody != null) {
        _updateTimer?.cancel();
        _invalidateAuthAttempt();
        widget.curOP.value = '';
        widget.cbLogin(authBody as Map<String, dynamic>);
        return;
      }
      final stateChanged = _stateMsg != stateMsg || _failedMsg != failedMsg;
      final newUrl = _url.isEmpty && url != null && url.isNotEmpty ? url : null;
      if (!stateChanged && newUrl == null) {
        return;
      }
      _setState(() {
        _stateMsg = stateMsg;
        _failedMsg = failedMsg;
        if (newUrl != null) {
          _url = newUrl;
        }
        if (failedMsg.isNotEmpty) {
          _invalidateAuthAttempt();
          widget.curOP.value = '';
          _updateTimer?.cancel();
        }
      });
      if (newUrl != null && failedMsg.isEmpty && !urlLaunched) {
        unawaited(_launchAuthUrl(newUrl));
      }
    }).catchError(
      (e) => _handleAuthFailure(
        authAttempt,
        e,
        'query account authentication',
      ),
    );
  }

  int _resetState() {
    _updateTimer?.cancel();
    _setState(() {
      _invalidateAuthAttempt();
      _stateMsg = _waitingAccountAuth;
      _failedMsg = '';
    });
    return _authAttempt;
  }
}
