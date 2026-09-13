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
  OutlineInputBorder border(Color color) => OutlineInputBorder(
      borderRadius: BorderRadius.circular(UiSpace.inputRadius),
      borderSide: BorderSide(color: color));

  Widget field(String label, TextEditingController controller, String? error,
      {FocusNode? focusNode, bool secret = false}) {
    final hasError = error != null && error.isNotEmpty;
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text(label, style: UiType.caption),
      const SizedBox(height: UiSpace.fieldLabelGap),
      SizedBox(
          height: UiSpace.controlHeight,
          child: TextField(
              controller: controller,
              focusNode: focusNode,
              obscureText: secret && !prefs.showPassword,
              enabled: !isInProgress,
              style: UiType.rowTitle
                  .copyWith(fontSize: 13, fontWeight: FontWeight.w400),
              decoration: InputDecoration(
                  filled: true,
                  fillColor: Colors.white,
                  isDense: true,
                  contentPadding: const EdgeInsets.symmetric(
                      horizontal: UiSpace.inputPaddingX, vertical: 8),
                  border: border(UiColor.inputBorder),
                  enabledBorder:
                      border(hasError ? UiColor.danger : UiColor.inputBorder),
                  focusedBorder:
                      border(hasError ? UiColor.danger : UiColor.primary),
                  suffixIcon: secret
                      ? IconButton(
                          iconSize: 16,
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(
                              minWidth: 28, minHeight: 28),
                          icon: Icon(
                              prefs.showPassword
                                  ? Icons.visibility_outlined
                                  : Icons.visibility_off_outlined,
                              color: UiColor.muted),
                          onPressed: () => setState(
                              () => prefs.showPassword = !prefs.showPassword))
                      : null))),
      SizedBox(
          height: UiSpace.panelErrorHeight,
          child: hasError
              ? Text(error,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: UiType.caption.copyWith(color: UiColor.danger))
              : null),
    ]);
  }

  Widget toggle(String label, bool value, ValueChanged<bool> onChanged) =>
      Row(children: [
        Expanded(
            child: Text(label,
                style: UiType.rowTitle.copyWith(fontWeight: FontWeight.w400))),
        SettingsSwitch(value: value, onChanged: isInProgress ? null : onChanged),
      ]);

  final busy = isInProgress || curOP.value.isNotEmpty;
  // DialogBuilder wants the CustomAlertDialog itself; UiDialog builds one.
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
                          style: UiType.caption
                              .copyWith(color: UiColor.textSecondary))),
                ]))),
        field(translate(DialogTextField.kUsernameTitle), username, usernameMsg,
            focusNode: userFocusNode),
        field(translate('Password'), password, passwordMsg, secret: true),
        toggle(zh ? '记住账号' : 'Remember account', prefs.rememberAccount,
            (v) => setState(() => prefs.rememberAccount = v)),
        const SizedBox(height: UiSpace.s2),
        toggle(zh ? '记住密码' : 'Remember password', prefs.rememberPassword,
            (v) => setState(() => prefs.rememberPassword = v)),
        if (isInProgress)
          const Padding(
              padding: EdgeInsets.only(top: UiSpace.s3),
              child: LinearProgressIndicator(minHeight: 2)),
        thirdAuth,
      ]),
      actions: [
        UiDialogAction.secondary('Cancel', close),
        UiDialogAction.primary('Login', busy ? null : onLogin),
      ]).build(context) as CustomAlertDialog;
}
