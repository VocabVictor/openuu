part of 'desktop_setting_page.dart';

/// The permanent password dialog on the desktop shell
/// (design-review-settings.md S21–S28): 400 wide, left-aligned title, labels
/// above the fields, a 4px strength bar, a vertical rule checklist, a
/// show-password toggle and the shell's buttons.
void _setPasswordDialogDesktop(
    {VoidCallback? notEmptyCallback,
    required bool localPasswordSet,
    required String statusTip,
    required int maxLength,
    required List rules}) {
  final p0 = TextEditingController();
  final p1 = TextEditingController();
  var err0 = '', err1 = '';
  var visible = false;
  gFFI.dialogManager.show((setState, close, context) {
    final zh = Localizations.localeOf(context).languageCode == 'zh';
    final pass = p0.text.trim();
    final canSubmit = pass.isNotEmpty || p1.text.trim().isNotEmpty;

    Future<void> submit() async {
      if (!canSubmit) return;
      setState(() {
        err0 = '';
        err1 = '';
      });
      if (pass.isNotEmpty) {
        final violations = rules.where((r) => !r.validate(pass));
        if (violations.isNotEmpty) {
          setState(() => err0 = violations.map((r) => r.name).join(', '));
          return;
        }
      }
      if (p1.text.trim() != pass) {
        setState(() => err1 = translate('The confirmation is not identical.'));
        return;
      }
      final ok = await bind.mainSetPermanentPasswordWithResult(password: pass);
      if (!ok) {
        setState(() => err0 = translate('Failed'));
        return;
      }
      if (pass.isNotEmpty) notEmptyCallback?.call();
      close();
    }

    Future<void> remove() async {
      final ok = await bind.mainSetPermanentPasswordWithResult(password: '');
      if (!ok) {
        setState(() => err0 = translate('Failed'));
        return;
      }
      close();
    }

    OutlineInputBorder border(Color color) => OutlineInputBorder(
        borderRadius: BorderRadius.circular(UiSpace.inputRadius),
        borderSide: BorderSide(color: color));

    Widget field(String label, TextEditingController controller, String error,
        {bool first = false}) {
      final length = controller.text.length;
      final showCounter = length > maxLength * .8;
      return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text(label, style: UiType.caption),
        const SizedBox(height: UiSpace.fieldLabelGap),
        SizedBox(
            height: UiSpace.controlHeight,
            child: TextField(
                controller: controller,
                obscureText: !visible,
                autofocus: first,
                maxLength: maxLength,
                onChanged: (_) => setState(() {
                      err0 = '';
                      err1 = '';
                    }),
                style: UiType.rowTitle
                    .copyWith(fontSize: 13, fontWeight: FontWeight.w400),
                decoration: InputDecoration(
                    counterText: '',
                    filled: true,
                    fillColor: Colors.white,
                    isDense: true,
                    contentPadding: const EdgeInsets.symmetric(
                        horizontal: UiSpace.inputPaddingX, vertical: 8),
                    border: border(UiColor.inputBorder),
                    enabledBorder: border(
                        error.isEmpty ? UiColor.inputBorder : UiColor.danger),
                    focusedBorder:
                        border(error.isEmpty ? UiColor.primary : UiColor.danger),
                    suffixIcon: first
                        ? IconButton(
                            iconSize: 16,
                            padding: EdgeInsets.zero,
                            constraints: const BoxConstraints(
                                minWidth: 28, minHeight: 28),
                            icon: Icon(
                                visible
                                    ? Icons.visibility_outlined
                                    : Icons.visibility_off_outlined,
                                color: UiColor.muted),
                            onPressed: () =>
                                setState(() => visible = !visible))
                        : null))),
        SizedBox(
            height: UiSpace.panelErrorHeight,
            child: Row(children: [
              Expanded(
                  child: Text(error,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: UiType.caption.copyWith(color: UiColor.danger))),
              if (showCounter)
                Text('$length/$maxLength',
                    style: UiType.caption.copyWith(color: UiColor.faint)),
            ])),
      ]);
    }

    final strength = pass.isEmpty ? 0.0 : estimatePasswordStrength(pass);
    final strengthColor = strength < .33
        ? UiColor.danger
        : strength < .67
            ? UiColor.warning
            : UiColor.success;
    final strengthLabel = pass.isEmpty
        ? ''
        : translate(strength < .33
            ? 'Weak'
            : strength < .67
                ? 'Medium'
                : 'Strong');

    return CustomAlertDialog(
        titlePadding: EdgeInsets.zero,
        contentBoxConstraints: const BoxConstraints(
            minWidth: UiSpace.dialogContentWidth,
            maxWidth: UiSpace.dialogContentWidth),
        content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                  height: UiSpace.dialogTitleHeight,
                  child: Row(children: [
                    Expanded(
                        child: Text(translate('Set Password'),
                            style:
                                UiType.sectionTitle.copyWith(fontSize: 16))),
                    IconButton(
                        iconSize: 16,
                        padding: EdgeInsets.zero,
                        constraints:
                            const BoxConstraints(minWidth: 28, minHeight: 28),
                        icon: const Icon(Icons.close, color: UiColor.muted),
                        onPressed: close),
                  ])),
              const SizedBox(height: UiSpace.s2),
              field(translate('Password'), p0, err0, first: true),
              ClipRRect(
                  borderRadius: BorderRadius.circular(2),
                  child: SizedBox(
                      height: 4,
                      child: Stack(children: [
                        Container(color: UiColor.settingsDivider),
                        FractionallySizedBox(
                            widthFactor: strength.clamp(0.0, 1.0),
                            child: Container(color: strengthColor)),
                      ]))),
              SizedBox(
                  height: UiSpace.panelErrorHeight,
                  child: Align(
                      alignment: Alignment.centerRight,
                      child: Text(strengthLabel,
                          style: UiType.caption
                              .copyWith(color: strengthColor)))),
              field(translate('Confirmation'), p1, err1),
              for (final rule in rules)
                Padding(
                    padding: const EdgeInsets.only(bottom: UiSpace.s1),
                    child: Row(children: [
                      Icon(
                          rule.validate(pass)
                              ? Icons.check_circle
                              : Icons.radio_button_unchecked,
                          size: 14,
                          color: rule.validate(pass)
                              ? UiColor.success
                              : UiColor.faint),
                      const SizedBox(width: 6),
                      Text(rule.name, style: UiType.caption),
                    ])),
              if (statusTip.isNotEmpty)
                Padding(
                    padding: const EdgeInsets.only(top: UiSpace.s2),
                    child: Text(statusTip, style: UiType.caption)),
              const SizedBox(height: UiSpace.s6),
              Row(mainAxisAlignment: MainAxisAlignment.end, children: [
                _secondaryButton('Cancel', close,
                    height: UiSpace.controlHeight),
                if (localPasswordSet) ...[
                  const SizedBox(width: UiSpace.s2),
                  _dangerButton(zh ? '清除' : 'Remove', remove,
                      height: UiSpace.controlHeight),
                ],
                const SizedBox(width: UiSpace.s2),
                _primaryButton('OK', canSubmit ? submit : null,
                    height: UiSpace.controlHeight),
              ]),
            ]),
        onSubmit: canSubmit ? submit : null,
        onCancel: close);
  });
}
