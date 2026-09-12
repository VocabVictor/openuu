import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher.dart';

import '../models/model.dart';
import '../models/platform_model.dart';

import 'connect.dart';
import 'ffi_options.dart';
import 'globals.dart';
import 'msgbox_parts.dart';
import 'overlay.dart';
import 'widgets_misc.dart';
import 'windows_misc.dart';

void msgBox(SessionID sessionId, String type, String title, String text,
    String link, OverlayDialogManager dialogManager,
    {bool? hasCancel,
    ReconnectHandle? reconnect,
    int? reconnectTimeout,
    VoidCallback? onSubmit,
    int? submitTimeout}) {
  dialogManager.dismissAll();
  if (type.contains('insecure-connection')) {
    Future<void> closeSession() async {
      await bind.sessionSetCommon(
        sessionId: sessionId,
        key: 'continue-insecure-connection',
        value: 'N',
      );
      dialogManager.dismissAll();
      closeConnection();
    }

    void continueSession() {
      unawaited(
        bind.sessionSetCommon(
          sessionId: sessionId,
          key: 'continue-insecure-connection',
          value: 'Y',
        ),
      );
      dialogManager.dismissAll();
    }

    dialogManager.show(
      (setState, close, context) => CustomAlertDialog(
        title: null,
        content: SelectionArea(child: msgboxContent(type, title, text)),
        actions: [
          dialogButton(
            'Continue',
            onPressed: continueSession,
            isOutline: true,
          ),
          dialogButton('Disconnect', onPressed: closeSession),
        ],
        onSubmit: closeSession,
        onCancel: closeSession,
      ),
      tag: '$sessionId-$type-$title-$text-$link',
    );
    return;
  }

  List<Widget> buttons = [];
  bool hasOk = false;
  submit() {
    dialogManager.dismissAll();
    if (onSubmit != null) {
      onSubmit.call();
    } else {
      // https://github.com/rustdesk/rustdesk/blob/5e9a31340b899822090a3731769ae79c6bf5f3e5/src/ui/common.tis#L263
      if (!type.contains("custom") && desktopType != DesktopType.portForward) {
        closeConnection();
      }
    }
  }

  cancel() {
    dialogManager.dismissAll();
  }

  jumplink() {
    if (link.startsWith('http')) {
      launchUrl(Uri.parse(link));
    }
  }

  if (type != "connecting" && type != "success" && !type.contains("nook")) {
    hasOk = true;
    late final Widget btn;
    if (submitTimeout != null) {
      btn = _CountDownButton(
        text: 'OK',
        second: submitTimeout,
        onPressed: submit,
        submitOnTimeout: true,
      );
    } else {
      btn = dialogButton('OK', onPressed: submit);
    }
    buttons.insert(0, btn);
  }
  hasCancel ??= !type.contains("error") &&
      !type.contains("nocancel") &&
      type != "restarting";
  if (hasCancel) {
    buttons.insert(
        0, dialogButton('Cancel', onPressed: cancel, isOutline: true));
  }
  if (type.contains("hasclose")) {
    buttons.insert(
        0,
        dialogButton('Close', onPressed: () {
          dialogManager.dismissAll();
        }));
  }
  if (reconnect != null &&
      title == "Connection Error" &&
      reconnectTimeout != null) {
    // `enabled` is used to disable the dialog button once the button is clicked.
    final enabled = true.obs;
    final button = Obx(() => _CountDownButton(
          text: 'Reconnect',
          second: reconnectTimeout,
          onPressed: enabled.isTrue
              ? () {
                  // Disable the button
                  enabled.value = false;
                  reconnect(dialogManager, sessionId, false);
                }
              : null,
        ));
    buttons.insert(0, button);
  }
  if (link.isNotEmpty) {
    buttons.insert(0, dialogButton('JumpLink', onPressed: jumplink));
  }
  dialogManager.show(
    (setState, close, context) => CustomAlertDialog(
      title: null,
      content: SelectionArea(child: msgboxContent(type, title, text)),
      actions: buttons,
      onSubmit: hasOk ? submit : null,
      onCancel: hasCancel == true ? cancel : null,
    ),
    tag: '$sessionId-$type-$title-$text-$link',
  );
}

class _CountDownButton extends StatefulWidget {
  _CountDownButton({
    Key? key,
    required this.text,
    required this.second,
    required this.onPressed,
    this.submitOnTimeout = false,
  }) : super(key: key);
  final String text;
  final VoidCallback? onPressed;
  final int second;
  final bool submitOnTimeout;

  @override
  State<_CountDownButton> createState() => _CountDownButtonState();
}

class _CountDownButtonState extends State<_CountDownButton> {
  late int _countdownSeconds = widget.second;

  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _startCountdownTimer();
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _startCountdownTimer() {
    _timer = Timer.periodic(Duration(seconds: 1), (timer) {
      if (_countdownSeconds <= 0) {
        timer.cancel();
        if (widget.submitOnTimeout) {
          widget.onPressed?.call();
        }
      } else {
        setState(() {
          _countdownSeconds--;
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return dialogButton(
      '${translate(widget.text)} (${_countdownSeconds}s)',
      onPressed: widget.onPressed,
      isOutline: true,
    );
  }
}
