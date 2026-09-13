part of 'login.dart';

// Local options of the desktop login dialog.
const _kRememberAccount = 'login-remember-account';
const _kRememberPassword = 'login-remember-password';
const _kSavedPassword = 'login-saved-password';

/// What the desktop dialog remembers between opens; the flags are applied
/// when a login succeeds.
class _LoginPrefs {
  bool rememberAccount =
      bind.mainGetLocalOption(key: _kRememberAccount) != 'N';
  bool rememberPassword =
      bind.mainGetLocalOption(key: _kRememberPassword) == 'Y';
  bool showPassword = false;

  String get savedPassword =>
      rememberPassword ? bind.mainGetLocalOption(key: _kSavedPassword) : '';

  Future<void> store(String username, String password) async {
    await bind.mainSetLocalOption(
        key: _kRememberAccount, value: rememberAccount ? 'Y' : 'N');
    await bind.mainSetLocalOption(
        key: _kRememberPassword, value: rememberPassword ? 'Y' : 'N');
    await bind.mainSetLocalOption(
        key: _kSavedPassword, value: rememberPassword ? password : '');
    if (!rememberAccount) {
      await bind.mainSetLocalOption(key: 'user_info', value: '');
    }
  }
}

/// The login dialog on the desktop shell, built on [UiDialog]: the account
/// server as a read-only line, labels above the fields, errors under them,
/// remember-account / remember-password switches, the third-party sign-in
/// options, and Cancel / Login as the only buttons.
CustomAlertDialog _desktopLoginDialog({
  required BuildContext context,
  required void Function(VoidCallback) setState,
  required VoidCallback close,
  required TextEditingController username,
  required TextEditingController password,
  required FocusNode userFocusNode,
  required String? usernameMsg,
  required String? passwordMsg,
  required bool isInProgress,
  required RxString curOP,
  required VoidCallback onLogin,
  required _LoginPrefs prefs,
  required Widget thirdAuth,
}) {
  final zh = Localizations.localeOf(context).languageCode == 'zh';
  final pal = UiColor.of(context);
  final typeset = UiType.of(context);

  final busy = isInProgress || curOP.value.isNotEmpty;
  return UiDialog(
      title: translate('Login'),
      onClose: close,
      body: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        futureBuilder(
            future: bind.mainGetApiServer(),
            hasData: (server) => Padding(
                padding: const EdgeInsets.only(bottom: UiSpace.s3),
                child: Row(children: [
                  Text(zh ? '账号服务器' : 'Account server',
                      style: UiType.caption),
                  const SizedBox(width: UiSpace.s2),
                  Expanded(
                      child: Text(server.toString(),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: typeset.caption
                              .copyWith(color: pal.textSecondary))),
                ]))),
        uiDialogField(pal, typeset,
            translate(DialogTextField.kUsernameTitle), username,
            error: usernameMsg,
            focusNode: userFocusNode,
            enabled: !isInProgress,
            onSubmitted: onLogin),
        uiDialogField(pal, typeset, translate('Password'), password,
            error: passwordMsg,
            enabled: !isInProgress,
            obscure: !prefs.showPassword,
            onToggleObscure: () =>
                setState(() => prefs.showPassword = !prefs.showPassword),
            onSubmitted: onLogin),
        uiDialogToggle(typeset, zh ? '记住账号' : 'Remember account',
            prefs.rememberAccount,
            isInProgress ? null : (v) => setState(() => prefs.rememberAccount = v)),
        const SizedBox(height: UiSpace.s2),
        uiDialogToggle(typeset, zh ? '记住密码' : 'Remember password',
            prefs.rememberPassword,
            isInProgress ? null : (v) => setState(() => prefs.rememberPassword = v)),
        if (isInProgress)
          const Padding(
              padding: EdgeInsets.only(top: UiSpace.s3),
              child: LinearProgressIndicator(minHeight: 2)),
        thirdAuth,
      ]),
      actions: [
        UiDialogAction.secondary('Cancel', close),
        UiDialogAction.primary('Login', busy ? null : onLogin),
      ]).alert(context);
}
