import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/consts.dart';

import '../../../common.dart';
import '../../../models/model.dart';
import '../../../models/platform_model.dart';
import 'audit_dialogs.dart';

void clientClose(SessionID sessionId, FFI ffi) async {
  if (allowAskForNoteAtEndOfConnection(ffi, true)) {
    if (await showConnEndAuditDialogCloseCanceled(ffi: ffi)) {
      return;
    }
    closeConnection();
  } else {
    msgBox(sessionId, 'info', 'Close', 'Are you sure to close the connection?',
        '', ffi.dialogManager);
  }
}

void changeWhiteList({Function()? callback}) async {
  final curWhiteList = await bind.mainGetOption(key: kOptionWhitelist);
  var newWhiteListField = curWhiteList == defaultOptionWhitelist
      ? ''
      : curWhiteList.split(',').join('\n');
  var controller = TextEditingController(text: newWhiteListField);
  var msg = "";
  var isInProgress = false;
  final isOptFixed = isOptionFixed(kOptionWhitelist);
  gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("IP Whitelisting")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(translate("whitelist_sep")),
          const SizedBox(
            height: 8.0,
          ),
          Text(translate("whitelist_cidr_tip")),
          const SizedBox(
            height: 8.0,
          ),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        decoration: InputDecoration(
                          errorText: msg.isEmpty ? null : translate(msg),
                        ),
                        controller: controller,
                        enabled: !isOptFixed,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
          const SizedBox(
            height: 4.0,
          ),
          // NOT use Offstage to wrap LinearProgressIndicator
          if (isInProgress) const LinearProgressIndicator(),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        if (!isOptFixed)
          dialogButton("Clear", onPressed: () async {
            await bind.mainSetOption(
                key: kOptionWhitelist, value: defaultOptionWhitelist);
            callback?.call();
            close();
          }, isOutline: true),
        if (!isOptFixed)
          dialogButton(
            "OK",
            onPressed: () async {
              setState(() {
                msg = "";
                isInProgress = true;
              });
              newWhiteListField = controller.text.trim();
              var newWhiteList = "";
              if (newWhiteListField.isEmpty) {
                // pass
              } else {
                final ips =
                    newWhiteListField.trim().split(RegExp(r"[\s,;\n]+"));
                // test ip
                final ipMatch = RegExp(
                    r"^(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)\.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9][0-9]?|0)(\/([1-9]|[1-2][0-9]|3[0-2])){0,1}$");
                final ipv6Match = RegExp(
                    r"^(((?:[0-9A-Fa-f]{1,4}))*((?::[0-9A-Fa-f]{1,4}))*::((?:[0-9A-Fa-f]{1,4}))*((?::[0-9A-Fa-f]{1,4}))*|((?:[0-9A-Fa-f]{1,4}))((?::[0-9A-Fa-f]{1,4})){7})(\/([1-9]|[1-9][0-9]|1[0-1][0-9]|12[0-8])){0,1}$");
                for (final ip in ips) {
                  if (!ipMatch.hasMatch(ip) && !ipv6Match.hasMatch(ip)) {
                    msg = "${translate("Invalid IP")} $ip";
                    setState(() {
                      isInProgress = false;
                    });
                    return;
                  }
                }
                newWhiteList = ips.join(',');
              }
              if (newWhiteList.trim().isEmpty) {
                newWhiteList = defaultOptionWhitelist;
              }
              await bind.mainSetOption(
                  key: kOptionWhitelist, value: newWhiteList);
              callback?.call();
              close();
            },
          ),
      ],
      onCancel: close,
    );
  });
}

Future<String> changeDirectAccessPort(
    String currentIP, String currentPort) async {
  final controller = TextEditingController(text: currentPort);
  await gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Change Local Port")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 8.0),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        keyboardType: TextInputType.number,
                        decoration: InputDecoration(
                            hintText: '21118',
                            isCollapsed: true,
                            prefix: Text('$currentIP : '),
                            suffix: IconButton(
                                padding: EdgeInsets.zero,
                                icon: const Icon(Icons.clear, size: 16),
                                onPressed: () => controller.clear())),
                        inputFormatters: [
                          FilteringTextInputFormatter.allow(RegExp(
                              r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                        ],
                        controller: controller,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () async {
          await bind.mainSetOption(
              key: kOptionDirectAccessPort, value: controller.text);
          close();
        }),
      ],
      onCancel: close,
    );
  });
  return controller.text;
}

Future<String> changeAutoDisconnectTimeout(String old) async {
  final controller = TextEditingController(text: old);
  await gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Timeout in minutes")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 8.0),
          Row(
            children: [
              Expanded(
                child: TextField(
                        maxLines: null,
                        keyboardType: TextInputType.number,
                        decoration: InputDecoration(
                            hintText: '10',
                            isCollapsed: true,
                            suffix: IconButton(
                                padding: EdgeInsets.zero,
                                icon: const Icon(Icons.clear, size: 16),
                                onPressed: () => controller.clear())),
                        inputFormatters: [
                          FilteringTextInputFormatter.allow(RegExp(
                              r'^([0-9]|[1-9]\d|[1-9]\d{2}|[1-9]\d{3}|[1-5]\d{4}|6[0-4]\d{3}|65[0-4]\d{2}|655[0-2]\d|6553[0-5])$')),
                        ],
                        controller: controller,
                        autofocus: true)
                    .workaroundFreezeLinuxMint(),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () async {
          await bind.mainSetOption(
              key: kOptionAutoDisconnectTimeout, value: controller.text);
          close();
        }),
      ],
      onCancel: close,
    );
  });
  return controller.text;
}
