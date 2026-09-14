part of 'login.dart';

const kAuthReqTypeOidc = 'oidc/';

Future<bool?>? _activeLoginDialog;

// call this directly
Future<bool?> loginDialog() {
  final activeDialog = _activeLoginDialog;
  if (activeDialog != null) {
    return activeDialog;
  }
  final dialog = _openLoginDialogOnce();
  _activeLoginDialog = dialog;
  return dialog;
}

Future<bool?> _openLoginDialogOnce() async {
  try {
    return await _openLoginDialog();
  } finally {
    _activeLoginDialog = null;
  }
}

Future<bool?> _openLoginDialog() async {
  final prefs = _LoginPrefs();
  var username =
      TextEditingController(text: UserModel.getLocalUserInfo()?['name'] ?? '');
  var password = TextEditingController(text: prefs.savedPassword);
  final userFocusNode = FocusNode()..requestFocus();
  Timer(Duration(milliseconds: 100), () => userFocusNode..requestFocus());

  String? usernameMsg;
  String? passwordMsg;
  var isInProgress = false;
  final oidcAuth = _OidcAuthController();
  final curOP = oidcAuth.curOP;
  // Track hover state for the close icon
  bool isCloseHovered = false;

  final loginOptions = [].obs;
  final loginOptionsError = Rxn<Object>();
  final loginOptionsInProgress = false.obs;
  fetchLoginOptions() async {
    loginOptionsInProgress.value = true;
    try {
      loginOptions.value = await UserModel.queryOidcLoginOptions();
      loginOptionsError.value = null;
    } catch (e) {
      debugPrint("queryOidcLoginOptions failed: $e");
      loginOptionsError.value = e;
    } finally {
      loginOptionsInProgress.value = false;
    }
  }

  Future.delayed(Duration.zero, fetchLoginOptions);

  final res = await gFFI.dialogManager.show<bool>((setState, close, context) {
    final pal = UiColor.of(context);
    username.addListener(() {
      if (usernameMsg != null) {
        setState(() => usernameMsg = null);
      }
    });

    password.addListener(() {
      if (passwordMsg != null) {
        setState(() => passwordMsg = null);
      }
    });

    onDialogCancel() {
      isInProgress = false;
      close(false);
    }

    handleLoginResponse(LoginResponse resp, bool storeIfAccessToken,
        void Function([dynamic])? close) async {
      switch (resp.type) {
        case HttpType.kAuthResTypeToken:
          if (resp.access_token != null) {
            if (storeIfAccessToken) {
              if (isWindows) await prefs.store(username.text, password.text);
              await bind.mainSetLocalOption(
                  key: 'access_token', value: resp.access_token!);
              await bind.mainSetOption(key: 'openuu-account-token', value: resp.access_token!);
              await bind.mainSetLocalOption(
                  key: 'user_info', value: jsonEncode(resp.user ?? {}));
            }
            if (close != null) {
              close(true);
            }
            return;
          }
          break;
        case HttpType.kAuthResTypeEmailCheck:
          bool? isEmailVerification;
          if (resp.tfa_type == null ||
              resp.tfa_type == HttpType.kAuthResTypeEmailCheck) {
            isEmailVerification = true;
          } else if (resp.tfa_type == HttpType.kAuthResTypeTfaCheck) {
            isEmailVerification = false;
          } else {
            passwordMsg = "Failed, bad tfa type from server";
          }
          if (isEmailVerification != null) {
            if (isMobile) {
              if (close != null) close(null);
              verificationCodeDialog(
                  resp.user, resp.secret, isEmailVerification);
            } else {
              setState(() => isInProgress = false);
              // Workaround for web, close the dialog first, then show the verification code dialog.
              // Otherwise, the text field will keep selecting the text and we can't input the code.
              // Not sure why this happens.
              final res = await verificationCodeDialog(
                  resp.user, resp.secret, isEmailVerification);
              if (res == true) {
                if (close != null) close(false);
                return;
              }
            }
          }
          break;
        default:
          passwordMsg = "Failed, bad response from server";
          break;
      }
    }

    onLogin() async {
      if (curOP.value.isNotEmpty || isInProgress) {
        return;
      }
      // validate
      if (username.text.isEmpty) {
        setState(() => usernameMsg = translate('Username missed'));
        return;
      }
      if (password.text.isEmpty) {
        setState(() => passwordMsg = translate('Password missed'));
        return;
      }
      curOP.value = 'rustdesk';
      setState(() => isInProgress = true);
      try {
        final resp = await gFFI.userModel.login(LoginRequest(
            username: username.text,
            password: password.text,
            id: await bind.mainGetMyId(),
            uuid: await bind.mainGetUuid(),
            autoLogin: true,
            type: HttpType.kAuthReqTypeAccount));
        await handleLoginResponse(resp, true, close);
      } on RequestException catch (err) {
        passwordMsg = translate(err.cause);
      } catch (err) {
        passwordMsg = "Unknown Error: $err";
      }
      curOP.value = '';
      setState(() => isInProgress = false);
    }

    thirdAuthWidget() => Obx(() {
          final error = loginOptionsError.value;
          final inProgress = loginOptionsInProgress.value;
          if (error != null) {
            return Column(
              children: [
                const SizedBox(height: 8.0),
                // NOT use Offstage to wrap LinearProgressIndicator
                if (inProgress) const LinearProgressIndicator(),
                if (!inProgress && error is! RequestException)
                  Text(
                    translate('network_error_tip'),
                    style: const TextStyle(fontSize: 12),
                    textAlign: TextAlign.center,
                  ),
                TextButton(
                  style: TextButton.styleFrom(
                    foregroundColor: Theme.of(context).colorScheme.primary,
                  ),
                  onPressed: inProgress ? null : fetchLoginOptions,
                  child: Text(translate('Retry')),
                ),
                if (!inProgress)
                  SelectableText(
                    error.toString(),
                    style: TextStyle(fontSize: 11, color: pal.danger),
                    textAlign: TextAlign.center,
                  ),
              ],
            );
          }
          return Offstage(
            offstage: loginOptions.isEmpty,
            child: Column(
              children: [
                const SizedBox(
                  height: 8.0,
                ),
                Center(
                    child: Text(
                  translate('or'),
                  style: TextStyle(fontSize: 16),
                )),
                const SizedBox(
                  height: 8.0,
                ),
                LoginWidgetOP(
                  ops: loginOptions
                      .map((e) => ConfigOP(op: e['name'], icon: e['icon']))
                      .toList(),
                  curOP: curOP,
                  startAuth: oidcAuth.start,
                  cancelAuth: oidcAuth.cancelCurrent,
                  canStartAuth: oidcAuth.canStart,
                  cbLogin: (Map<String, dynamic> authBody) async {
                    LoginResponse? resp;
                    try {
                      // access_token is already stored in the rust side.
                      resp =
                          gFFI.userModel.getLoginResponseFromAuthBody(authBody);
                    } catch (e) {
                      debugPrint(
                          'Failed to parse account login response');
                    }
                    close(true);

                    if (resp != null) {
                      handleLoginResponse(resp, false, null);
                    }
                  },
                ),
              ],
            ),
          );
        });

    if (isWindows) {
      return _desktopLoginDialog(
          context: context,
          setState: setState,
          close: onDialogCancel,
          username: username,
          password: password,
          userFocusNode: userFocusNode,
          usernameMsg: usernameMsg,
          passwordMsg: passwordMsg,
          isInProgress: isInProgress,
          curOP: curOP,
          onLogin: onLogin,
          prefs: prefs,
          thirdAuth: thirdAuthWidget());
    }
    final title = Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          translate('Login'),
        ).marginOnly(top: MyTheme.dialogPadding),
        MouseRegion(
          onEnter: (_) => setState(() => isCloseHovered = true),
          onExit: (_) => setState(() => isCloseHovered = false),
          child: InkWell(
            child: Icon(
              Icons.close,
              size: 25,
              // No need to handle the branch of null.
              // Because we can ensure the color is not null when debug.
              color: isCloseHovered
                  ? pal.onPrimary
                  : Theme.of(context)
                      .textTheme
                      .titleLarge
                      ?.color
                      ?.withOpacity(0.55),
            ),
            onTap: onDialogCancel,
            hoverColor: pal.danger,
            borderRadius: BorderRadius.circular(5),
          ),
        ).marginOnly(top: 10, right: 15),
      ],
    );
    final titlePadding = EdgeInsets.fromLTRB(MyTheme.dialogPadding, 0, 0, 0);

    final dialog = CustomAlertDialog(
      title: isWindows
          ? Row(children: [
              const BrandIcon(size: 28),
              const SizedBox(width: 12),
              const Expanded(
                  child: Text('OpenUU',
                      style: TextStyle(
                          fontSize: 24, fontWeight: FontWeight.w600))),
              IconButton(
                  onPressed: onDialogCancel,
                  tooltip: translate('Close'),
                  icon: const Icon(Icons.close, size: 20)),
            ])
          : title,
      titlePadding:
          isWindows ? const EdgeInsets.fromLTRB(28, 20, 16, 0) : titlePadding,
      contentBoxConstraints: BoxConstraints(
          minWidth: isWindows ? 360 : 400,
          maxWidth: isWindows ? 360 : double.infinity),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          if (isWindows) ...[
            const SizedBox(height: 4),
            Text(translate('Login'),
                style:
                    const TextStyle(fontSize: 20, fontWeight: FontWeight.w600)),
            const SizedBox(height: 8),
            Text(
                Localizations.localeOf(context).languageCode == 'zh'
                    ? '登录账号，连接你的设备'
                    : 'Sign in to connect to your devices',
                style: TextStyle(fontSize: 13, color: pal.muted)),
            const SizedBox(height: 16),
          ],
          const SizedBox(
            height: 8.0,
          ),
          LoginWidgetUserPass(
            username: username,
            pass: password,
            usernameMsg: usernameMsg,
            passMsg: passwordMsg,
            isInProgress: isInProgress,
            curOP: curOP,
            onLogin: onLogin,
            userFocusNode: userFocusNode,
          ),
          thirdAuthWidget(),
        ],
      ),
      onCancel: onDialogCancel,
      onSubmit: onLogin,
    );
    return dialog;
  }).whenComplete(oidcAuth.close);

  if (res != null) {
    await UserModel.updateOtherModels();
  }

  return res;
}
