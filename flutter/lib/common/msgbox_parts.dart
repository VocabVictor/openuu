import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';
import 'package:url_launcher/url_launcher.dart';


import 'ffi_options.dart';
import 'globals.dart';
import 'my_theme.dart';
import 'overlay.dart';
import 'package:flutter_hbb/models/model.dart';

// TODO
// - Remove argument "contentPadding", no need for it, all should look the same.
// - Remove "required" for argument "content". See simple confirm dialog "delete peer", only title and actions are used. No need to "content: SizedBox.shrink()".
// - Make dead code alive, transform arguments "onSubmit" and "onCancel" into correspondenting buttons "ConfirmOkButton", "CancelButton".
class CustomAlertDialog extends StatelessWidget {
  const CustomAlertDialog(
      {Key? key,
      this.title,
      this.titlePadding,
      this.theme,
      required this.content,
      this.actions,
      this.contentPadding,
      this.contentBoxConstraints = const BoxConstraints(maxWidth: 500),
      this.onSubmit,
      this.onCancel})
      : super(key: key);

  final ThemeData? theme;
  final Widget? title;
  final EdgeInsetsGeometry? titlePadding;
  final Widget content;
  final List<Widget>? actions;
  final double? contentPadding;
  final BoxConstraints contentBoxConstraints;
  final Function()? onSubmit;
  final Function()? onCancel;

  @override
  Widget build(BuildContext context) {
    if (theme != null) return Theme(data: theme!, child: Builder(builder: _buildDialog));
    return _buildDialog(context);
  }

  Widget _buildDialog(BuildContext context) {
    // request focus
    FocusScopeNode scopeNode = FocusScopeNode();
    Future.delayed(Duration.zero, () {
      if (!scopeNode.hasFocus) scopeNode.requestFocus();
    });
    bool tabTapped = false;
    if (isAndroid) gFFI.invokeMethod("enable_soft_keyboard", true);

    return FocusScope(
      node: scopeNode,
      autofocus: true,
      onKey: (node, key) {
        if (key.logicalKey == LogicalKeyboardKey.escape) {
          if (key is RawKeyDownEvent) {
            onCancel?.call();
          }
          return KeyEventResult.handled; // avoid TextField exception on escape
        } else if (!tabTapped &&
            onSubmit != null &&
            (key.logicalKey == LogicalKeyboardKey.enter ||
                key.logicalKey == LogicalKeyboardKey.numpadEnter)) {
          if (key is RawKeyDownEvent) onSubmit?.call();
          return KeyEventResult.handled;
        } else if (key.logicalKey == LogicalKeyboardKey.tab) {
          if (key is RawKeyDownEvent) {
            scopeNode.nextFocus();
            tabTapped = true;
          }
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: AlertDialog(
          scrollable: true,
          title: title,
          content: ConstrainedBox(
            constraints: contentBoxConstraints,
            child: content,
          ),
          actions: actions,
          titlePadding: titlePadding ?? MyTheme.dialogTitlePadding(),
          contentPadding:
              MyTheme.dialogContentPadding(actions: actions is List),
          actionsPadding: MyTheme.dialogActionsPadding(),
          buttonPadding: MyTheme.dialogButtonPadding),
    );
  }
}

Widget createDialogContent(String text) {
  final RegExp linkRegExp = RegExp(r'(https?://[^\s]+)');
  bool hasLink = linkRegExp.hasMatch(text);

  // Early return: no link, use default theme color
  if (!hasLink) {
    return SelectableText(text, style: const TextStyle(fontSize: 15));
  }

  final List<TextSpan> spans = [];
  int start = 0;

  linkRegExp.allMatches(text).forEach((match) {
    if (match.start > start) {
      spans.add(TextSpan(text: text.substring(start, match.start)));
    }
    spans.add(TextSpan(
      text: match.group(0) ?? '',
      style: const TextStyle(
        color: Colors.blue,
        decoration: TextDecoration.underline,
      ),
      recognizer: TapGestureRecognizer()
        ..onTap = () {
          String linkText = match.group(0) ?? '';
          linkText = linkText.replaceAll(RegExp(r'[.,;!?]+$'), '');
          launchUrl(Uri.parse(linkText));
        },
    ));
    start = match.end;
  });

  if (start < text.length) {
    spans.add(TextSpan(text: text.substring(start)));
  }

  return SelectableText.rich(
    TextSpan(
      style: const TextStyle(fontSize: 15),
      children: spans,
    ),
  );
}

Color? _msgboxColor(String type) {
  if (type == "input-password" || type == "custom-os-password") {
    return Color(0xFFAD448E);
  }
  if (type.contains("success")) {
    return Color(0xFF32bea6);
  }
  if (type.contains("error") || type == "re-input-password") {
    return Color(0xFFE04F5F);
  }
  return Color(0xFF2C8CFF);
}

Widget msgboxIcon(String type) {
  IconData? iconData;
  if (type.contains("error") || type == "re-input-password") {
    iconData = Icons.cancel;
  }
  if (type.contains("success")) {
    iconData = Icons.check_circle;
  }
  if (type == "wait-uac" || type == "wait-remote-accept-nook") {
    iconData = Icons.hourglass_top;
  }
  if (type == 'on-uac' || type == 'on-foreground-elevated') {
    iconData = Icons.admin_panel_settings;
  }
  if (type.contains('info')) {
    iconData = Icons.info;
  }
  if (iconData != null) {
    return Icon(iconData, size: 50, color: _msgboxColor(type))
        .marginOnly(right: 16);
  }

  return Offstage();
}

// title should be null
Widget msgboxContent(String type, String title, String text) {
  String translateText(String text) {
    if (text.indexOf('Failed') == 0 && text.indexOf(': ') > 0) {
      List<String> words = text.split(': ');
      for (var i = 0; i < words.length; ++i) {
        words[i] = translate(words[i]);
      }
      text = words.join(': ');
    } else {
      List<String> words = text.split(' ');
      if (words.length > 1 && words[0].endsWith('_tip')) {
        words[0] = translate(words[0]);
        final rest = text.substring(words[0].length + 1);
        text = '${words[0]} ${translate(rest)}';
      } else {
        text = translate(text);
      }
    }
    return text;
  }

  return Row(
    children: [
      msgboxIcon(type),
      Expanded(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              translate(title),
              style: TextStyle(fontSize: 21),
            ).marginOnly(bottom: 10),
            createDialogContent(translateText(text)),
          ],
        ),
      ),
    ],
  ).marginOnly(bottom: 12);
}

void msgBoxCommon(OverlayDialogManager dialogManager, String title,
    Widget content, List<Widget> buttons,
    {bool hasCancel = true}) {
  dialogManager.show((setState, close, context) => CustomAlertDialog(
        title: Text(
          translate(title),
          style: TextStyle(fontSize: 21),
        ),
        content: content,
        actions: buttons,
        onCancel: hasCancel ? close : null,
      ));
}
